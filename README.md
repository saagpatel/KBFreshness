# KB Freshness Detector

A Rust + React application that continuously monitors knowledge base articles for freshness, validates links, captures visual changes via screenshots, and correlates support tickets to identify outdated documentation.

**Goal:** Proactive KB maintenance. Target metrics:
- 80% of articles <90 days old
- 95% links working  
- 15% ticket deflection within 6 months

## Features

### Core Capabilities
- ✅ **Article Sync** - Pulls articles from Confluence Cloud with metadata
- ✅ **Link Validation** - Concurrent HEAD requests to detect broken links
- ✅ **Screenshot Capture** - Headless Chrome screenshots with visual drift detection
- ✅ **Ticket Correlation** - Jira ticket analysis with keyword extraction and fuzzy matching
- ✅ **LLM Suggestions** - Optional Ollama integration for update recommendations
- ✅ **Health Dashboard** - Real-time article health monitoring (Green/Yellow/Red)

### Automated Jobs
- **Daily 2 AM** - Full freshness scan (Confluence sync + link validation)
- **Daily 3 AM** - Screenshot cleanup (>30 day old)
- **Weekly Mon 4 AM** - Screenshot capture for visual drift
- **Weekly Mon 5 AM** - Jira ticket analysis and correlation

## Quick Start

See full documentation in README.md for:
- Prerequisites and API keys
- Configuration
- Deployment options
- Troubleshooting

**Development:**
```bash
# Backend
cargo run --features "screenshots,tickets"

# Frontend  
cd frontend && npm install && npm run dev
```

Access dashboard at http://localhost:5173
