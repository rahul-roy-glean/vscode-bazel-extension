#!/bin/bash

# This script sets up a connection to a GKE cluster through a bastion host.
# It:
# 1. Establishes an IAP tunnel to the bastion host
# 2. Sets up kubectl credentials for the cluster
#
# Usage:
#   ./connect_gke.sh                    # Uses current gcloud project
#   ./connect_gke.sh -p PROJECT_ID      # Specify a different project
#   ./connect_gke.sh -z ZONE            # Specify a different zone
#   ./connect_gke.sh -c CLUSTER_NAME    # Specify a different cluster
#
# After running, you'll need to set the proxy in your shell:
#   export HTTPS_PROXY=localhost:7971
#
# Press Ctrl+C when done to clean up the connection.

set -euo pipefail

# Function to display usage
usage() {
    echo "Usage: $0 [options]"
    echo "Options:"
    echo "  -p, --project PROJECT_ID    GCP project ID (default: current gcloud project)"
    echo "  -z, --zone ZONE            GCP zone (default: us-central1-a)"
    echo "  -c, --cluster CLUSTER      GKE cluster name (default: glean-cluster)"
    echo "  -h, --help                 Display this help message"
    exit 1
}

# Default values
ZONE="us-central1-a"
CLUSTER="glean-cluster"
PROJECT=$(gcloud config get project)

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -p|--project)
            PROJECT="$2"
            shift 2
            ;;
        -z|--zone)
            ZONE="$2"
            shift 2
            ;;
        -c|--cluster)
            CLUSTER="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
done

# Check if project is set
if [ -z "$PROJECT" ]; then
    echo "Error: No project ID found. Please set a project using 'gcloud config set project PROJECT_ID' or use -p option"
    usage
fi

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check required commands
for cmd in gcloud kubectl; do
    if ! command_exists $cmd; then
        echo "Error: $cmd is not installed"
        exit 1
    fi
done

# Function to start bastion connection
start_bastion() {
    echo "Starting bastion connection..."
    gcloud compute ssh --zone "$ZONE" --tunnel-through-iap bastion -- -N -L 7971:localhost:8888 &
    BASTION_PID=$!
    echo "Bastion connection started with PID: $BASTION_PID"
    # Wait for the connection to establish
    sleep 5
}

# Function to setup kubectl
setup_kubectl() {
    echo "Setting up kubectl configuration..."
    gcloud container clusters get-credentials "$CLUSTER" --project "$PROJECT" --zone "$ZONE" --internal-ip
    echo "Kubectl configuration completed"
}

# Function to cleanup on exit
cleanup() {
    if [ ! -z "$BASTION_PID" ]; then
        echo "Cleaning up bastion connection..."
        kill $BASTION_PID 2>/dev/null
    fi
    exit 0
}

# Set up trap for cleanup
trap cleanup EXIT INT TERM

# Main execution
echo "Connecting to GKE cluster in project: $PROJECT"

# Start bastion connection
start_bastion

# Setup kubectl
setup_kubectl

echo "Setup complete! To use kubectl commands, run:"
echo "export HTTPS_PROXY=localhost:7971"
echo "Press Ctrl+C to disconnect when you're done"

# Keep the script running to maintain the connection
wait $BASTION_PID