param(
  [string]$ImageName = "anchor-builder:0.30.1",
  [string]$RepoPath = "${PWD}\sol-token-mill-interface"
)

Write-Host "Building Docker image $ImageName (this may take several minutes)..."
# Build image from repo root
docker build -t $ImageName "$PWD"

Write-Host "Running container to build Anchor programs (mounting repo)..."
# Run container, mount repo and run anchor build
docker run --rm -v "$RepoPath:/workspace" -w /workspace $ImageName 
# Run the anchor build inside the container (run as root inside container)
Write-Host "Starting anchor build inside container..."
$cmd = "cd /workspace && anchor build --skip-lint"
# Execute the build
docker run --rm -v "$RepoPath:/workspace" -w /workspace $ImageName bash -lc $cmd | Tee-Object build_docker_output.txt

Write-Host 'Build output saved to build_docker_output.txt on host (if any).'
