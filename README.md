# Order Matching Engine

A toy price-time-priority matching engine for a prediction market.

## The hard parts

### Why a shared log instead of shared state

"Price-time priority" across N API servers needs a single definition of "time." Wall-clock
time is unreliable (skew, NTP jumps, concurrent writes). A Redis Stream gives a total order
of submissions: **stream position is our time axis.**

Any number of API servers can `XADD` concurrently; Redis serializes them. The matcher is
the single consumer and the single writer to the book, so there are no locks inside the
engine itself.

### Why the matcher is single-writer

The book is an in-memory `BTreeMap<Price, VecDeque<Order>>` per side. One writer means the
matching code (`src/book.rs`) is pure and synchronous — no `Arc`, no `Mutex`, no async —
which is why the five unit tests in `tests/matching.rs` can exercise it directly without
any I/O mocking.

### Why POST returns before the match

`POST /orders` assigns an ID, pushes to the stream, and returns. The fill arrives later via
the WebSocket feed. Synchronous match-on-submit would require a correlation map between
order IDs and response futures for a toy with no user-visible benefit.

### What breaks if the matcher dies

The book is in memory. On matcher restart, the book is empty — replayed orders would
rebuild it from the stream, but the current implementation reads from `last_id = "0"` only
on fresh start, not after a crash. Production would checkpoint `last_id` and the book to
disk; this is documented as out of scope.



## Questions

**1. How is double-matching prevented across API instances?**
API servers never match they only `XADD` to a Redis stream and return. A single matcher.
process consumes the stream, so there is exactly one book and one writer. Stream position
gives a total order across instances, which defines "time" for price-time priority.

**2. Why this order-book data structure?**
`BTreeMap<Price, VecDeque<Order>>` per side. BTreeMap gives O(log n) best-price lookup
(`keys().next()` / `keys().next_back()`); VecDeque gives O(1) FIFO within a level. Together
that's price-then-time priority with no locks, because the matcher owns the booksingle-threaded.

**3. What breaks first under real load?**
The matcher: one process, one core, one stream consumer. `POST`s stay fast but fills fall behind the cursor and `GET /orderbook` goes stale. Close second: Redis pub/sub is lossy, so WS clients silently miss fills during any forwarder reconnect.

**4. What would you build next with 4 more hours?**
Checkpoint matcher `last_id` + book so restarts don't replay every fill. Then `DELETE/orders/:id` (the stream model handles this cleanly, just another event type). Then aproperty test that hammers concurrent `POST`s and asserts the no-double-match invariant.



## Run

```
docker compose up --build
```

This starts Redis, one matcher, and two API servers on ports `8080` and `8081`.

### Submit to one server, read from the other

```
curl -X POST localhost:8080/orders \
  -H 'content-type: application/json' \
  -d '{"side":"Sell","price":100,"qty":5}'

curl -X POST localhost:8081/orders \
  -H 'content-type: application/json' \
  -d '{"side":"Buy","price":100,"qty":5}'

curl localhost:8080/orderbook
```

### Watch fills on the WebSocket feed

```
use this api : ws://localhost:8080/ws
```
