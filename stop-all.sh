#!/bin/bash
echo "Stopping BSV Bank services..."
pkill -f deposit-service
pkill -f interest-engine
pkill -f lending-service
echo "✓ All services stopped"
