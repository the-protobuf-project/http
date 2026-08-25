// Command music-server serves the music catalog over HTTP.
//
// The Go counterpart of the Rust example's binary, over the route table
// protoc-gen-http emitted for this runtime from the same protos. Serving both
// and getting the same answers is the point: the conformance tests assert it,
// and this binary is how you check it by hand.
//
//	go run ./cmd/music-server
//	curl http://127.0.0.1:8080/v1/artists/miles/tracks/so-what
package main

import (
	"context"
	"errors"
	"flag"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"time"

	"github.com/the-protobuf-project/grpc-gateway-rs/examples/music-go"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/middleware"
	"github.com/the-protobuf-project/grpc-gateway-rs/netadapter/middleware/builtin"
)

// shutdownGrace is how long in-flight requests have to finish on a signal.
const shutdownGrace = 5 * time.Second

func main() {
	addr := flag.String("addr", "127.0.0.1:8080", "Address to listen on.")
	flag.Parse()

	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelInfo}))
	health := builtin.Healthz()

	gateway := music.NewAdapter(
		music.SeededCatalog(),
		netadapter.WithLogger(logger),
		// Every policy below is selected by what a method means rather than by
		// what it is called, so a method added to the protos lands in the right
		// buckets without this list being touched.
		netadapter.Use(builtin.NewRecovery(logger)),
		netadapter.Use(builtin.NewLogging(logger)),
		netadapter.Use(builtin.NewDeadline(30*time.Second, music.Domain())),
		netadapter.Use(builtin.Direct()),
		netadapter.Use(builtin.PermissiveCORS()),
		netadapter.UseFor(
			builtin.NewIdempotency(music.NewRequestIDs(), logger),
			middleware.Mutating(),
		),
	)

	server := &http.Server{
		Addr:              *addr,
		Handler:           health.Wrap(gateway),
		ReadHeaderTimeout: 10 * time.Second,
	}

	go func() {
		logger.Info("serving", "addr", *addr, "health", health.Path())
		if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			logger.Error("server failed", "error", err)
			os.Exit(1)
		}
	}()

	// A clean shutdown rather than an abrupt exit: an in-flight streaming
	// response that is cut off looks exactly like the truncation this gateway
	// uses to signal failure, and an operator restarting a process should not
	// have to wonder which one they are looking at.
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
	defer stop()
	<-ctx.Done()

	logger.Info("shutting down", "grace", shutdownGrace)
	timeout, cancel := context.WithTimeout(context.Background(), shutdownGrace)
	defer cancel()
	if err := server.Shutdown(timeout); err != nil {
		logger.Error("shutdown failed", "error", err)
	}
}
