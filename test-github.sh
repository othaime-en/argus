#!/bin/bash

# GitHub API Test Script
# This script will help you verify what the GitHub API is actually returning

echo "========================================"
echo "GitHub Actions API Test"
echo "========================================"
echo ""

# Check if GITHUB_TOKEN is set
if [ -z "$GITHUB_TOKEN" ]; then
    echo "ERROR: GITHUB_TOKEN environment variable is not set"
    echo "Please set it with: export GITHUB_TOKEN='your_token_here'"
    exit 1
fi

echo "✓ GITHUB_TOKEN is set"
echo ""

# Test 1: Your personal repos
echo "Test 1: Fetching workflow runs for othaime-en/secret-rotator"
echo "URL: https://api.github.com/repos/othaime-en/secret-rotator/actions/runs?per_page=5"
echo ""

curl -s \
  -H "Authorization: token $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  "https://api.github.com/repos/othaime-en/secret-rotator/actions/runs?per_page=5" \
  | jq '.workflow_runs | length' 2>/dev/null || echo "Error parsing response"

echo ""
echo "----------------------------------------"
echo ""

# Test 2: Your second repo
echo "Test 2: Fetching workflow runs for othaime-en/py-chaos-agent"
echo "URL: https://api.github.com/repos/othaime-en/py-chaos-agent/actions/runs?per_page=5"
echo ""

curl -s \
  -H "Authorization: token $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  "https://api.github.com/repos/othaime-en/py-chaos-agent/actions/runs?per_page=5" \
  | jq '.workflow_runs | length' 2>/dev/null || echo "Error parsing response"

echo ""
echo "----------------------------------------"
echo ""

# Test 3: rust-lang/rust (should have many runs)
echo "Test 3: Fetching workflow runs for rust-lang/rust"
echo "URL: https://api.github.com/repos/rust-lang/rust/actions/runs?per_page=5"
echo ""

curl -s \
  -H "Authorization: token $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  "https://api.github.com/repos/rust-lang/rust/actions/runs?per_page=5" \
  | jq '.workflow_runs | length' 2>/dev/null || echo "Error parsing response"

echo ""
echo "========================================"
echo ""
echo "If all tests show '0', your repositories don't have any workflow runs."
echo "If tests show numbers > 0, then the API is working but ARGUS has a bug."
echo ""
echo "To see the full JSON response for debugging, run:"
echo "  curl -H \"Authorization: token \$GITHUB_TOKEN\" \\"
echo "       -H \"Accept: application/vnd.github+json\" \\"
echo "       -H \"X-GitHub-Api-Version: 2022-11-28\" \\"
echo "       \"https://api.github.com/repos/othaime-en/secret-rotator/actions/runs?per_page=5\" | jq ."