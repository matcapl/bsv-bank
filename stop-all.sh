#!/bin/bash
echo "Stopping BSV Bank services..."
pkill -f deposit-service
pkill -f interest-engine
echo "✓ All services stopped"
