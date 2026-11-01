# Arena Allocation and Bump Memory Architecture

In data transformation pipelines, HTML/JSON parsing trees, AST generation, and per-request lifecycles, applications often allocate thousands of small individual structs. Allocating and deallocating each object independently creates allocator overhead, heap fragmentation, and CPU cache misses.

---

## 1. How Bump Allocation Works

An Arena / Bump Allocator (such as `bumpalo`) reserves a single contiguous chunk of memory. Allocating a new object simply advances ("bumps") an offset pointer:
- **Allocation Speed**: Pointer addition ($O(1)$) with zero locks and zero system calls.
- **Cache Locality**: Objects allocated sequentially sit next to each other in CPU L1/L2 cache lines.
- **Deallocation Speed**: The entire arena is reclaimed in a single operation ($O(1)$) by resetting the pointer (`bump.reset()`), bypassing individual `free()` system calls.

---

## 2. Request-Scoped Arena Allocation with `bumpalo`

Add `bumpalo` to `Cargo.toml`:
```toml
[dependencies]
bumpalo = { version = "3.16", features = ["collections"] }
```

Use a bump arena for short-lived AST nodes or batch objects:

```rust
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;

pub struct AstNode<'a> {
    pub tag: &'a str,
    pub children: BumpVec<'a, &'a AstNode<'a>>,
}

pub fn parse_syntax_tree_in_arena<'a>(input: &'a str, arena: &'a Bump) -> &'a AstNode<'a> {
    // Allocates inside linear bump memory chunk
    let root = arena.alloc(AstNode {
        tag: "root",
        children: BumpVec::new_in(arena),
    });

    // Sub-allocations are trivial pointer advances
    for token in input.split_whitespace() {
        let child = arena.alloc(AstNode {
            tag: token,
            children: BumpVec::new_in(arena),
        });
        root.children.push(child);
    }

    root
}

pub fn execute_batch_workload() {
    let mut arena = Bump::with_capacity(64 * 1024); // 64KB arena

    for batch_item in 0..10_000 {
        let _tree = parse_syntax_tree_in_arena("item data tokens", &arena);
        // Process _tree...

        // RESET: Frees all allocated nodes instantly in O(1) without system calls
        arena.reset();
    }
}
```

---

## 3. When to Use Arena Allocation

| Use Arena Allocation (`bumpalo`) | Use Standard Heap Allocator |
| :--- | :--- |
| Request-scoped parsing (AST, DOM, GraphQL query trees) | Long-lived state surviving across multiple requests |
| Graph/tree algorithms with complex cross-references | Objects with differing, non-uniform lifecycles |
| Batch workers processing millions of ephemeral records | Shared state inside `Arc<RwLock<T>>` |

---

## 4. Production Checklist

- [ ] Replaced per-node `Box::new()` in AST/tree parsing with `arena.alloc()`.
- [ ] Pre-allocated arena capacity (`Bump::with_capacity(bytes)`) matching average request size.
- [ ] Recycled arena per batch using `arena.reset()` to eliminate heap fragmentation.
