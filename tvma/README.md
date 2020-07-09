
# TVMA - Toy Vulkan Memory Allocator

This crate provides very simple memory allocator for Vulkan written purely in Rust.
*Ash crate is used as Vulkan API*.

Supports 3 allocation strategies for 5 memory usage patterns.

* Dedicated - creates memory object from device for every allocation.
  This strategy is used for very large allocations.

* Linear - allocates memory consecutively from large memory objects.
  Does not allows memory reuse, but minimizes overhead and is very fast - one atomic operation unless new memory object is needed.
  Memory is returned to the device when all allocations from memory object are freed.
  Used for staging buffers.

* Chunked - allocation strategy that allocates blocks of memory from equally-sized chunks
  which themselves are allocated from the same allocator until specified treshold is reached and new memory object is allocated.
  May add significant initial overhead.
  *NOTE: Consider to start with smaller treshold for memory object allocation*

Allocator picks most appropriate memory type among available filtered by mask specified to `alloc` function
(this mask is typically comes from requirements for the resource).

Allocated blocks share memory object (except dedicated allocations).
This adds complexity to memory mapping because Vulkan requires that only one range from any given memory object is mapped.
To avoid this complexity TVMA maps all host visible memory allocated for chunked and linear suballocator.
Then `Block::map` just returns stored pointer and `Block::unmap` is no-op.
