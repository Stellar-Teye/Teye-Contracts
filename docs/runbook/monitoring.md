# Monitoring & Alerting Setup

This playbook extends `docs/monitoring.md` with production-operable procedures.

## 1. Monitoring Stack Bootstrap

1. Start stack:
   - `cd scripts/monitor`
   - `docker-compose up -d`
2. Verify services:
   - Grafana: `http://localhost:3000`
   - Prometheus: `http://localhost:9090`
   - Alertmanager: `http://localhost:9093`
3. Confirm containers are healthy:
   - `docker-compose ps`

## 2. Health Probe Configuration

1. Configure probe target:

```bash
export RPC_URL="https://rpc-futurenet.stellar.org"
export CONTRACT_ID="<DEPLOYED_CONTRACT_ID>"
```

2. Run health probe:

```bash
./scripts/monitor/health_check.sh
```

3. Expected output includes:
- `RPC is healthy.` or `RPC responded, assuming healthy for now.`
- `Health check completed successfully. 0 Errors.`

## 3. Alert Rules and Thresholds

### Existing Rules (`scripts/monitor/prometheus/alerts.yml`)

- `RPCDown` (critical): exporter unavailable for `> 1m`
- `HighErrorRate` (warning): `rate(contract_transaction_errors_total[5m]) > 5`
- `SlowRPCResponse` (warning): `contract_rpc_latency_seconds > 2.0` for `3m`

### Required Operational Thresholds

Use or extend Prometheus rules to include the following:

1. Contract invocation failures
- Warning: failure ratio `> 2%` over 5m
- Critical: failure ratio `> 5%` over 5m

2. Gas consumption anomalies
- Warning: per-method gas usage `> 2x` 7-day baseline for 10m
- Critical: per-method gas usage `> 3x` 7-day baseline for 10m

3. Unusual access patterns (possible breach)
- Warning: `EMRG_GRT` grants or auth-denied spikes `> 3x` baseline for 15m
- Critical: sustained spike `> 5x` baseline for 15m

4. Storage utilization approaching limits
- Warning: persistent storage utilization `>= 80%`
- Critical: persistent storage utilization `>= 90%`

## 4. Add/Update Alert Rules

1. Edit `scripts/monitor/prometheus/alerts.yml`
2. Add threshold rules for the four categories above
3. Reload Prometheus:
   - `curl -X POST http://localhost:9090/-/reload`
4. Validate active alerts page:
   - `http://localhost:9090/alerts`

## 5. Dashboard Setup (Grafana)

Pre-provisioned files:
- Datasource: `scripts/monitor/grafana/provisioning/datasources/prometheus.yml`
- Dashboard provisioning: `scripts/monitor/grafana/provisioning/dashboards/dashboards.yml`
- Dashboard JSON: `scripts/monitor/grafana/dashboards/contract_health.json`

Steps:

1. Log in to Grafana (`admin/admin` default; rotate in production).
2. Confirm `Prometheus` datasource is healthy.
3. Open `Contract Health Dashboard`.
4. Add panels for:
- Invocation success/failure rate by contract
- p50/p95/p99 RPC latency
- Gas per method + baseline delta
- Unauthorized access/event anomaly counts
- Storage usage and forecast to threshold breach

## 6. On-Call Response Targets

- Critical alerts: acknowledge in `<= 5 minutes`, mitigation started in `<= 15 minutes`
- Warning alerts: acknowledge in `<= 30 minutes`

## 7. Monitoring Scripts Reference

### `scripts/monitor/health_check.sh`

Purpose:
- Basic RPC + contract liveness check.

Usage:

```bash
RPC_URL=<rpc> CONTRACT_ID=<id> ./scripts/monitor/health_check.sh
```

### `scripts/monitor/prometheus/prometheus.yml`

Purpose:
- Scrape configuration and alert file loading.

Current target:
- `contract-health-exporter` at `host.docker.internal:8000/metrics`

### `scripts/monitor/prometheus/alerts.yml`

Purpose:
- Alert rule definitions and severities.

### `scripts/monitor/docker-compose.yml`

Purpose:
- Local/ops stack orchestration for Prometheus, Alertmanager, Grafana.

## 8. Validation Checklist

- [ ] Alert rules loaded without parse errors
- [ ] Dashboard panels update with live data
- [ ] Test alert fired and delivered to Alertmanager receiver
- [ ] Contract ID tags updated after each deployment
- [ ] Runbook links included in on-call documentation
