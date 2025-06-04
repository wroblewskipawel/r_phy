# r_phy

Learning Rust and exploring its capabilities in context of game engine implementation.

Currently implemented:
* `math` - simple linear algebra crate
* `type_kit` - generic utility types
  * defining and operating on TypeLists
  * enforcing uniform creation-destruction pattern for low-level primitives (eg. Vulkan objects) for which Rust drop pattern is often unfeasible
  * type downcasting and upcasting with typeId checks in Debug build
  * GenerationalVector implementation, allowing for replacement of reference types with collectios indexable with safe handles
    * integration with TypeList - GenerationalVector type lists with support of multiple items borrow by indexing with lists of item handles
    * integration with type guard and drop guard patterns for safe interoperation with stored objects
* `graphics` - collection of common types and frontend interface definition for renderer implementation
* `vulkan` - Vulkan renderer implementation
  * simple deferred rendering pipeline implementation
  * leveraging type lists for enabling compile time type safety for low level Vulkan objects

`sandbox` crate acts as example and prving ground for implemented concepts, default target executable with `cargo run`
