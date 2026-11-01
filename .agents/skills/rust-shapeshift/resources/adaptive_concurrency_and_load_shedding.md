# Adaptive Concurrency Limits and Load Shedding

Fixed rate limits and static thread pools break down when downstream resources (database connection pools, third-party microservices) degrade in latency. When response times increase from 10ms to 2 seconds, requests queue up in memory, memory consumption balloons, and the service crashes under cascading timeouts.

---

## 1. Little's Law and Dynamic Concurrency

Little's Law defines the relationship between concurrency ($L$), arrival rate ($\lambda$), and latency ($W$):
$$L = \lambda \times W$$

When system latency ($W$) doubles due to downstream pressure, the service must **reduce** allowed in-flight requests ($L$) to maintain stable memory and prevent queuing explosions.

---

## 2. Adaptive Concurrency Middleware with Tower

Instead of an arbitrary hardcoded limit (e.g. `10,000 requests`), use dynamic concurrency limits based on gradient feedback or Vegas algorithms:

```rust
use tower::limit::ConcurrencyLimit;
use tower::ServiceBuilder;
use axum::{routing::get, Router, http::StatusCode};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct DynamicLoadShedder {
    current_inflight: Arc<AtomicUsize>,
    max_inflight_ceiling: usize,
}

impl DynamicLoadShedder {
    pub fn new(max_inflight_ceiling: usize) -> Self {
        Self {
            current_inflight: Arc::new(AtomicUsize::new(0)),
            max_inflight_ceiling,
        }
    }

    pub fn try_acquire(&self) -> Option<InFlightGuard> {
        let current = self.current_inflight.fetch_add(1, Ordering::SeqCst);
        if current >= self.max_inflight_ceiling {
            self.current_inflight.fetch_sub(1, Ordering::SeqCst);
            None // Over capacity: Shed load immediately
        } else {
            Some(InFlightGuard {
                counter: Arc::clone(&self.current_inflight),
            })
        }
    }
}

pub struct InFlightGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}
```

---

## 3. Fast-Fail Load Shedding in Axum

Shed excess load immediately with `503 Service Unavailable` or `429 Too Many Requests` before allocating request bodies or queuing worker tasks:

```rust
use axum::{
    middleware::{self, Next},
    response::{IntoResponse, Response},
    extract::{State, Request},
    http::StatusCode,
};

pub async fn load_shedding_middleware(
    State(shedder): State<DynamicLoadShedder>,
    req: Request,
    next: Next,
) -> Response {
    match shedder.try_acquire() {
        Some(_guard) => {
            // Under capacity limit: proceed with processing
            next.run(req).await
        }
        None => {
            // Over capacity limit: Fast-fail to protect server memory & queues
            (
                StatusCode::SERVICE_UNAVAILABLE,
                [("Retry-After", "2")],
                "Server load limit exceeded. Please retry.",
            ).into_response()
        }
    }
}
```

---

## 4. Upstream Backpressure & Graceful Degradation

Combine load shedding with priority tiers:
1. **Critical Path** (e.g., `/healthz`, `/api/v1/checkout`): Allowed with reserved headroom buffer.
2. **Non-Critical Path** (e.g., `/api/v1/analytics`, `/metrics`, background scans): Shed first when in-flight counts exceed 70% of ceiling.

```rust
pub fn is_sheddable_route(path: &str) -> bool {
    !path.starts_with("/healthz") && !path.starts_with("/api/v1/checkout")
}
```

---

## 5. Production Checklist

- [ ] Replaced static unbounded queues with bounded in-flight ceilings.
- [ ] Configured fast-fail `503 Service Unavailable` with `Retry-After` header before parsing bodies.
- [ ] Priority routes (checkout, health probes) bypass shedding guards.
- [ ] Drop guard ensures in-flight counters decrement even on unexpected panic or cancellation.
