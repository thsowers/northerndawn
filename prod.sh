#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

echo "==> Installing frontend dependencies..."
cd frontend && npm install

echo "==> Building frontend..."
npm run build
cd ..

echo "==> Building backend (release)..."
cargo build --release -p backend

echo "==> Starting backend..."
cd backend && ../target/release/backend
