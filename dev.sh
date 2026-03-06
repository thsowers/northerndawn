#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

echo "==> Installing frontend dependencies..."
cd frontend && npm install && cd ..

echo "==> Starting backend (cargo run)..."
cd backend && cargo run &
BACKEND_PID=$!
cd ..

echo "==> Starting frontend (vite dev)..."
cd frontend && npm run dev &
FRONTEND_PID=$!
cd ..

trap 'echo "==> Shutting down..."; kill $BACKEND_PID $FRONTEND_PID 2>/dev/null; wait' INT TERM

echo "==> Dev servers running. Press Ctrl+C to stop."
wait
