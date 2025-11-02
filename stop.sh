#!/bin/bash
echo "Stopping BSV Bank..."
pkill -f deposit-service
docker-compose down
echo "All services stopped"