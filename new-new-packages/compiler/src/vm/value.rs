// TODO: We should have a notion of a self-contained value that is not only
// valid in the context of a VM / fiber. These self-contained values could then
// be sent through channels between multiple reference-counted heaps, for
// example ones running concurrently logically, on other cores, or on different
// computers.
