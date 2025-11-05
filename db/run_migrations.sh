#!/bin/bash
set -e

echo "Running database migrations..."

# Load environment
source ../../.env 2>/dev/null || true

DB_URL=${DATABASE_URL:-"postgresql://a:@localhost:5432/bsv_bank"}

# Run migration
psql "$DB_URL" -f migrations/001_initial_schema.sql

echo "✓ Migrations completed successfully"
