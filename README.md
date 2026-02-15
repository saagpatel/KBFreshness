# KB Freshness Detector

A production-ready Rust + React application that automatically monitors knowledge base articles for freshness, validates links, captures visual changes via screenshots, and correlates support tickets to identify outdated documentation.

**The Problem:** Knowledge bases decay over time. Links break, screenshots become outdated, content becomes stale, and support tickets pile up for issues that should be documented. Manual tracking doesn't scale.

**The Solution:** Automated continuous monitoring with AI-assisted insights that flags problematic articles before they impact users.

## Why Use This?

### The Business Case
- **Reduce Support Load**: Identify gaps between KB content and actual user issues by correlating support tickets
- **Improve User Experience**: Catch broken links and outdated screenshots before users do
- **Enforce Accountability**: Track article age and ownership with visual health indicators
- **Data-Driven Decisions**: Metrics on article health, link validity, and update frequency

### Target Metrics
- 80% of articles less than 90 days old
- 95% of links working
- 15% ticket deflection improvement within 6 months

### What Makes This Different
- **Automated Freshness Tracking**: Daily scans of Confluence spaces with configurable thresholds
- **Broken Link Detection**: Concurrent validation of all article links with redirect handling
- **Visual Drift Detection**: Screenshot comparison to catch UI changes in referenced applications
- **Smart Ticket Correlation**: Uses Jaro-Winkler similarity and keyword extraction to match support tickets with relevant articles
- **AI-Powered Suggestions**: Optional LLM integration (Ollama) generates update recommendations based on ticket patterns
- **Production Hardened**: Comprehensive error handling, retry logic, configurable rate limiting

## What You Can Do With It

### Primary Use Cases

1. **Proactive KB Maintenance**
   - Schedule daily scans to flag stale articles
   - Set per-article freshness thresholds (or use global 90-day default)
   - Review health dashboard to prioritize updates

2. **Link Health Monitoring**
   - Automatically detect broken links across all KB articles
   - Track redirect chains and SSL issues
   - Get notified before users encounter 404s

3. **Visual Change Detection**
   - Weekly screenshot capture of referenced applications
   - Hash-based comparison to detect UI changes
   - Timeline view showing visual evolution

4. **Support Ticket Analysis**
   - Correlate Jira tickets with KB articles using fuzzy matching
   - Identify documentation gaps where tickets cluster
   - Get LLM-generated suggestions for article improvements

5. **Compliance & Auditing**
   - Track who reviewed articles and when
   - Export metrics on KB health over time
   - Enforce freshness policies with automated flagging

### Example Workflows

**Workflow 1: Weekly KB Triage**
```
1. View dashboard filtered by "Red" health status
2. Review articles flagged as stale (>90 days)
3. Check ticket patterns to see what users are asking
4. Mark articles as reviewed or schedule updates
```

**Workflow 2: New Product Launch**
```
1. Set custom freshness thresholds for launch-related articles (e.g., 30 days)
2. Enable screenshot capture for new UI flows
3. Monitor ticket correlation to catch documentation gaps
4. Use LLM suggestions to quickly update content
```

**Workflow 3: Broken Link Cleanup**
```
1. Filter articles by "broken links" status
2. Bulk review link check results
3. Update or remove dead links
4. Re-scan to verify fixes
```

## Features

### Core Capabilities
- ✅ **Article Sync** - Pulls articles from Confluence Cloud with metadata (title, URL, last modified, author)
- ✅ **Link Validation** - Concurrent HEAD requests with configurable rate limiting to detect broken links
- ✅ **Screenshot Capture** - Headless Chrome screenshots with perceptual hash-based visual drift detection
- ✅ **Ticket Correlation** - Jira ticket analysis with keyword extraction and fuzzy matching (Jaro-Winkler similarity)
- ✅ **LLM Suggestions** - Optional Ollama integration for AI-generated update recommendations
- ✅ **Health Dashboard** - Real-time article health monitoring with Green/Yellow/Red status indicators
- ✅ **Review Tracking** - Mark articles as reviewed with reviewer name and timestamp
- ✅ **Stats API** - Aggregated metrics endpoint for monitoring and reporting

### Automated Jobs
- **Daily 2 AM** - Full freshness scan (Confluence sync + link validation)
- **Daily 3 AM** - Screenshot cleanup (removes screenshots >30 days old)
- **Weekly Monday 4 AM** - Screenshot capture for visual drift detection
- **Weekly Monday 5 AM** - Jira ticket analysis and correlation

All job schedules are configurable in code. Failed jobs automatically retry 3 times with 5-minute delays.

## How to Use It

### Prerequisites

1. **System Requirements**
   - Rust 1.70+ (see `rust-toolchain.toml`)
   - Node.js 18+ (for frontend)
   - PostgreSQL 14+
   - Chrome/Chromium (for screenshot feature)

2. **API Access**
   - **Confluence Cloud** - Email + API token with read access to target space
   - **Jira Cloud** (optional) - Email + API token with read access to projects
   - **Ollama** (optional) - Local instance running for LLM suggestions

### Configuration

Create a `.env` file in the project root:

```bash
# Required
DATABASE_URL=postgres://kb:kb@localhost:5432/kb_freshness
CONFLUENCE_EMAIL=your-email@example.com
CONFLUENCE_API_TOKEN=your-confluence-api-token
CONFLUENCE_SPACE_KEY=KB  # Your Confluence space key

# Server Configuration
BIND_ADDRESS=127.0.0.1:3001  # Use 0.0.0.0:3001 for Docker/cloud
CORS_ALLOWED_ORIGIN=http://localhost:5173  # Frontend URL

# Optional - Jira Integration
JIRA_EMAIL=your-email@example.com
JIRA_API_TOKEN=your-jira-api-token
JIRA_BASE_URL=https://yourcompany.atlassian.net
JIRA_PROJECT_KEY=SUP  # Project key to analyze

# Optional - LLM Integration
OLLAMA_BASE_URL=http://localhost:11434
OLLAMA_MODEL=llama3.2:3b

# Optional - Performance Tuning
SCREENSHOT_WAIT_SECS=3  # Seconds to wait for page load before screenshot
SCREENSHOT_DELAY_MS=500  # Delay between screenshots to avoid rate limits
MAX_CONCURRENT_LINK_CHECKS=10  # Max concurrent link validation requests
```

### Local Development Setup

**1. Start PostgreSQL**
```bash
docker-compose up -d postgres
```

**2. Run Database Migrations**
```bash
cargo install sqlx-cli
sqlx migrate run
```

**2b. (Optional) Seed Local Demo Data**
```bash
psql "$DATABASE_URL" -f scripts/seed_dev_data.sql
```
Use this only for local/demo environments. It inserts synthetic sample articles and link checks.

**3. Start Backend** (with all features enabled)
```bash
cargo run --features "screenshots,tickets"
```

**4. Start Frontend**
```bash
cd frontend
npm install
npm run dev
```

**5. Access Dashboard**
- Frontend: http://localhost:5173
- Backend API: http://localhost:3001
- Health Check: http://localhost:3001/health

### Docker Deployment

```bash
# Build and run with docker-compose
docker-compose up -d

# View logs
docker-compose logs -f backend

# Stop services
docker-compose down
```

**Environment Variables for Production:**
- Set `BIND_ADDRESS=0.0.0.0:3001`
- Set `CORS_ALLOWED_ORIGIN` to your frontend domain
- Configure all required API tokens
- Use a production PostgreSQL instance (not the docker-compose one)

### Feature Flags

The backend supports optional features via Cargo feature flags:

- `screenshots` - Enables screenshot capture and visual drift detection (requires Chrome)
- `tickets` - Enables Jira ticket correlation and LLM suggestions

**Examples:**
```bash
# Minimal build (no screenshots, no tickets)
cargo run

# Screenshots only
cargo run --features screenshots

# Tickets only
cargo run --features tickets

# All features
cargo run --features "screenshots,tickets"
```

## API Endpoints

### Articles
- `GET /api/articles` - List articles with filtering, sorting, pagination
  - Query params: `health`, `search`, `sort_by`, `sort_order`, `page`, `limit`
- `GET /api/articles/stats` - Get aggregated health statistics
- `GET /api/articles/:id` - Get single article with full details
- `POST /api/articles/:id/review` - Mark article as reviewed

### Scans
- `GET /api/scans` - List scan runs with status
- `POST /api/scans/trigger` - Trigger manual scan

### Screenshots
- `GET /api/screenshots/:article_id` - Get all screenshots for an article
- `GET /api/screenshots/:id/image` - Get screenshot image data

### Health
- `GET /health` - Health check endpoint (database + configuration status)

## Project Structure

```
KBFreshnessDetector/
├── src/
│   ├── api/           # HTTP endpoints and request handlers
│   ├── db/            # Database queries and models
│   ├── jobs/          # Scheduled background jobs
│   ├── sources/       # External integrations (Confluence, Jira)
│   ├── config.rs      # Environment configuration
│   ├── error.rs       # Error types and handling
│   └── main.rs        # Application entry point
├── frontend/
│   ├── src/
│   │   ├── components/  # React components
│   │   ├── pages/       # Page layouts
│   │   ├── api/         # API client
│   │   └── types/       # TypeScript types
│   └── vite.config.ts
├── migrations/        # Database schema migrations
├── Cargo.toml        # Rust dependencies
└── docker-compose.yml # Local development stack
```

## Database Schema

### Core Tables
- `articles` - KB articles from Confluence with health metadata
- `link_checks` - Link validation results for each article
- `screenshots` - Screenshot metadata and perceptual hashes
- `screenshot_images` - Binary image data
- `ticket_patterns` - Correlated support tickets with similarity scores
- `scan_runs` - Audit log of automated scans

### Health Calculation
Articles receive health scores based on:
- **Age**: Days since last modification vs. threshold (default 90 days)
- **Broken Links**: Percentage of broken links
- **Visual Changes**: Screenshot hash differences

**Health Status:**
- 🟢 **Green**: Fresh content, no broken links, stable screenshots
- 🟡 **Yellow**: Approaching staleness or minor issues
- 🔴 **Red**: Stale content, broken links, or significant visual drift

## Monitoring & Operations

### Health Check
```bash
curl http://localhost:3001/health
```

Returns database connectivity and configuration status.

### Logs
Structured logging with `tracing`:
- Errors: Critical failures requiring attention
- Warnings: Non-fatal issues (e.g., failed link checks)
- Info: Job execution, scan results
- Debug: Detailed request/response data (set `RUST_LOG=debug`)

### Metrics to Track
- Article health distribution (Green/Yellow/Red counts)
- Link check success rate
- Screenshot capture success rate
- Ticket correlation hit rate
- Job execution times

## Troubleshooting

**"Database connection failed"**
- Verify `DATABASE_URL` is correct
- Check PostgreSQL is running: `docker-compose ps`
- Run migrations: `sqlx migrate run`

**"Confluence sync returns no articles"**
- Verify `CONFLUENCE_SPACE_KEY` matches your space
- Check API token has read permissions
- Confirm space has published articles

**"Screenshots not capturing"**
- Ensure Chrome/Chromium is installed
- Build with `--features screenshots`
- Check `SCREENSHOT_WAIT_SECS` allows enough load time

**"Ticket correlation returns no results"**
- Verify `JIRA_PROJECT_KEY` is correct
- Check similarity threshold (default 0.8, configurable in code)
- Ensure Jira API token has read access

**"Job not running on schedule"**
- Check server logs for scheduler initialization
- Verify timezone settings (jobs use UTC)
- Manually trigger via `/api/scans/trigger` to test

## Development

### Running Tests
```bash
cargo test
```

### Code Review
This codebase has been comprehensively reviewed and hardened:
- ✅ No `unwrap()` in production paths
- ✅ Proper error handling with `thiserror`
- ✅ SQL injection protection via parameter validation
- ✅ Input sanitization on all user-facing endpoints
- ✅ Configurable rate limiting and request timeouts
- ✅ Retry logic for transient failures

### Contributing
1. Follow Rust best practices (no `unwrap()` in production code)
2. Add tests for new features
3. Run `cargo fmt` and `cargo clippy` before committing
4. Update this README for new configuration options

## License

MIT

## Support

For issues, questions, or feature requests, please open a GitHub issue.

---

**Built with:** Rust, Axum, SQLx, Tokio, React, TypeScript, TailwindCSS, Headless Chrome, Ollama
