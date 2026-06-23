import { Elysia } from "elysia";
import { Registry, Gauge, Counter, Histogram } from "prom-client";

const registry = new Registry();

// ── PRD System Metrics ──────────────────────────────

const rss = new Gauge({
  name: "process_resident_set_size_bytes",
  help: "Resident Set Size — total physical RAM used by the process",
  registers: [registry],
});

const jscMemory = new Gauge({
  name: "bun_edge_format_memory",
  help: "JavaScriptCore heap memory allocation (Bun-specific)",
  registers: [registry],
});

const eventloopLag = new Gauge({
  name: "eventloop_lag_seconds",
  help: "Event loop lag — delay of the event loop for synchronous blocking detection",
  registers: [registry],
});

// ── Extended System Metrics ─────────────────────────

const cpuTotal = new Counter({
  name: "process_cpu_seconds_total",
  help: "Total user + system CPU time spent (seconds)",
  registers: [registry],
});

const heapBytes = new Gauge({
  name: "process_heap_bytes",
  help: "Heap memory usage (used / total)",
  labelNames: ["state"] as const,
  registers: [registry],
});

const externalMemory = new Gauge({
  name: "process_external_memory_bytes",
  help: "External memory usage (C++ objects bound to JS objects)",
  registers: [registry],
});

const uptime = new Gauge({
  name: "process_start_time_seconds",
  help: "Start time of the process since unix epoch (seconds)",
  registers: [registry],
});

// ── HTTP RED Metrics ────────────────────────────────

const httpRequestsTotal = new Counter({
  name: "http_requests_total",
  help: "Total number of HTTP requests",
  labelNames: ["method", "path", "status"] as const,
  registers: [registry],
});

const httpRequestDuration = new Histogram({
  name: "http_request_duration_seconds",
  help: "HTTP request duration in seconds",
  labelNames: ["method", "path", "status"] as const,
  buckets: [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
  registers: [registry],
});

const inflight = new Gauge({
  name: "http_requests_in_flight",
  help: "Number of HTTP requests currently being processed (active)",
  labelNames: ["method"] as const,
  registers: [registry],
});

// ── Timer tracking ──────────────────────────────────

const requestTimers = new WeakMap<Request, ReturnType<typeof httpRequestDuration.startTimer>>();

// ── Label extraction ────────────────────────────────

function getLabels(ctx: Record<string, any>): { method: string; path: string; status: string } {
  const method = ctx.request?.method ?? "UNKNOWN";
  const path = (ctx.route as string) || ctx.path || "unknown";

  let status = "500";
  if (ctx.set?.status) {
    status = ctx.set.status.toString();
  } else if (
    ctx.response &&
    typeof ctx.response === "object" &&
    "code" in ctx.response &&
    typeof ctx.response.code === "number"
  ) {
    status = ctx.response.code.toString();
  }

  return { method, path, status };
}

// ── Elysia Plugin ───────────────────────────────────

function registerMetrics() {
  return new Elysia({ name: "prometheus" })
    .onRequest((ctx) => {
      inflight.inc({ method: ctx.request.method });
      requestTimers.set(ctx.request, httpRequestDuration.startTimer());
    })
    .onAfterResponse({ as: "global" }, (ctx) => {
      if (ctx.path === "/metrics") return;
      inflight.dec({ method: ctx.request.method });
      const endTimer = requestTimers.get(ctx.request);
      if (!endTimer) return;
      const labels = getLabels(ctx);
      httpRequestsTotal.inc(labels);
      endTimer(labels);
    })
    .onError({ as: "global" }, (ctx) => {
      if (ctx.path === "/metrics") return;
      inflight.dec({ method: ctx.request.method });
    });
}

// ── Collector Logic ─────────────────────────────────

let cpuPrevious = { user: 0, system: 0 };
let processStartTime = Date.now();

function measureEventLoopLag(): Promise<number> {
  return new Promise((resolve) => {
    const start = performance.now();
    setTimeout(() => {
      resolve((performance.now() - start) / 1000);
    }, 0);
  });
}

async function collect() {
  // RSS
  rss.set(process.memoryUsage().rss);

  // JSC heap proxy via process.memoryUsage
  jscMemory.set(process.memoryUsage().heapUsed);

  // Event loop lag
  const lag = await measureEventLoopLag();
  eventloopLag.set(lag);

  // CPU time delta
  const cpu = process.cpuUsage();
  const userDelta = cpu.user - cpuPrevious.user;
  const sysDelta = cpu.system - cpuPrevious.system;
  cpuPrevious = cpu;
  if (userDelta > 0 || sysDelta > 0) {
    cpuTotal.inc((userDelta + sysDelta) / 1_000_000);
  }

  // Heap used / total
  const heap = process.memoryUsage();
  heapBytes.set({ state: "used" }, heap.heapUsed);
  heapBytes.set({ state: "total" }, heap.heapTotal);

  // External memory
  externalMemory.set(heap.external);
}

let interval: ReturnType<typeof setInterval> | null = null;

function startSystemMetrics(intervalMs = 15_000) {
  processStartTime = Date.now();
  uptime.set(Math.floor(processStartTime / 1000));
  cpuPrevious = process.cpuUsage();
  collect();
  interval = setInterval(collect, intervalMs);
}

function stopSystemMetrics() {
  if (interval) {
    clearInterval(interval);
    interval = null;
  }
}

export {
  registry,
  registerMetrics,
  startSystemMetrics,
  stopSystemMetrics,
};
