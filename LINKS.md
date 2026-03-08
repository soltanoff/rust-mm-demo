# Awesome Concurrency

- [The Free Lunch Is Over: A Fundamental Turn Toward Concurrency in Software](http://www.gotw.ca/publications/concurrency-ddj.htm)
  by Herb Sutter
- [Concurrency is not Parallelism](https://go.dev/talks/2012/waza.slide#1) by Rob Pike

---

## Memory [Consistency] Models

### Intro

- [How to Make a Multiprocessor Computer That Correctly Executes Multiprocess Programs](https://www.microsoft.com/en-us/research/publication/make-multiprocessor-computer-correctly-executes-multiprocess-programs/)
  by Leslie Lamport

#### By Russ Cox

- [Hardware MM](https://research.swtch.com/hwmm)
- [Programming Language MM](https://research.swtch.com/plmm)

### Axiomatic

- [Weak Ordering – A New Definition](https://dl.acm.org/doi/10.1145/325096.325100) by Sarita Adve & Mark Hill
- [Memory Models: A Case For Rethinking Parallel Languages and Hardware](https://dl.acm.org/doi/10.1145/1787234.1787255)
  by Sarita Adve and Hans Boehm
- [Foundations of the C++ Concurrency Memory Model](https://www.semanticscholar.org/paper/Foundations-of-the-C%2B%2B-concurrency-memory-model-Boehm-Adve/29ead6d3bd248b6845a6d311a4d92f629c87fbd6)
  by Hans Boehm, Sarita Adve
- [The Problem of Programming Language Concurrency Semantics](https://www.cl.cam.ac.uk/~jp622/the_problem.pdf) by Mark
  Batty et al
- [Repairing Sequential Consistency in C/C++11](https://plv.mpi-sws.org/scfix/paper.pdf) by Lahav et
  al + [P0668R5: Revising the C++ memory model](https://www.open-std.org/jtc1/sc22/wg21/docs/papers/2018/p0668r5.html)
- [Weak Memory Consistency](https://people.mpi-sws.org/~viktor/wmc/) by Ori Lahav and Viktor Vafeiadis

#### Promising Semantics

- [A Promising Semantics for Relaxed-Memory Concurrency](https://sf.snu.ac.kr/promise-concurrency/)
- [Promising 2.0: Global Optimizations in Relaxed Memory Concurrency](https://www.cs.tau.ac.il/~orilahav/papers/promising2.pdf)

### Hardware MM

- [Relaxed Memory](https://www.cl.cam.ac.uk/~pes20/multicore-sewell-notes.pdf) by Peter Sewell & Shaked Flur

#### Arch

- [A Better x86 Memory Model: x86-TSO](https://www.cl.cam.ac.uk/~pes20/weakmemory/x86tso-paper.tphols.pdf)
- [A Tutorial Introduction to the ARM and POWER Relaxed Memory Models](https://www.cl.cam.ac.uk/~pes20/ppc-supplemental/test7.pdf)
- [Simplifying ARM Concurrency: Multicopy-Atomic Axiomatic and Operational Models for ARMv8](https://www.cl.cam.ac.uk/~pes20/armv8-mca/armv8-mca-draft.pdf)

### Barriers

- [Memory Barriers: a Hardware View for Software Hackers](http://www.puppetmastertrading.com/images/hwViewForSwHackers.pdf)
  by Paul McKenney
- [Linux Kernel Memory Barriers](https://github.com/torvalds/linux/blob/master/Documentation/memory-barriers.txt), [Quotes of the week](https://lwn.net/Articles/575835/)

### Linux Kernel MM

- https://github.com/torvalds/linux/tree/master/tools/memory-model
- [Explanation](https://github.com/torvalds/linux/blob/master/tools/memory-model/Documentation/explanation.txt)
- [Frightening Small Children and Disconcerting Grown-ups: Concurrency in the Linux Kernel](http://pauillac.inria.fr/~maranget/papers/asplos2018.pdf)

### C++ MM Index

- [sequential consistency for DRF programs](https://eel.is/c++draft/intro.races#note-21), [sequential consistency vs weak ordering](https://eel.is/c++draft/atomics.order#7)
- [S (synchronization order)](https://eel.is/c++draft/atomics.order#4)
- [sequenced before](https://eel.is/c++draft/intro.execution#def:sequenced_before)
- [synchronizes with](https://eel.is/c++draft/atomics.order#2), [release sequence](https://eel.is/c++draft/intro.races#def:release_sequence), [fences](https://eel.is/c++draft/atomics.fences)
- [modification order](https://eel.is/c++draft/intro.races#def:modification_order), [read the last value](https://eel.is/c++draft/atomics.order#10)
- [coherence-ordered before](https://eel.is/c++draft/atomics.order#3)
- [stores eventually visible](https://eel.is/c++draft/atomics.order#11)
- [happens-before](https://eel.is/c++draft/intro.races#def:happens_before)
- [strongly happens-before](https://eel.is/c++draft/intro.races#def:strongly_happens_before)
- [visible side effect](https://eel.is/c++draft/intro.races#def:side_effects,visible)
- [conflict](https://eel.is/c++draft/intro.races#def:conflict), [data race](https://eel.is/c++draft/intro.races#def:data_race)
- [out-of-thin-air](https://eel.is/c++draft/atomics.order#8), [self-fulfilling prophecy](https://eel.is/c++draft/atomics.order#9)
- [seq-cst-fix](https://eel.is/c++draft/atomics.order#6)

### PL MM

- Java Memory Model
  - [JLS §17.4 – Memory Model](https://docs.oracle.com/javase/specs/jls/se25/html/jls-17.html#jls-17.4)
  - [JLS §8.3.1.4 – volatile Fields](https://docs.oracle.com/javase/specs/jls/se25/html/jls-8.html#jls-8.3.1.4)
- [Go Memory Model](https://golang.org/ref/mem), [Don't be clever](https://golang.org/ref/mem#tmp_1), [Updating the Go MM](https://research.swtch.com/gomm)
- [LLVM Atomic Instructions and Concurrency Guide](https://llvm.org/docs/Atomics.html)
- [Multicore OCaml Memory Model](https://v2.ocaml.org/manual/memorymodel.html), [Bounding Data Races in Space and Time](https://kcsrk.info/papers/pldi18-memory.pdf)
- [Rust Memory model](https://doc.rust-lang.org/reference/memory-model.html), [The Rustonomicon / Atomics](https://doc.rust-lang.org/nomicon/atomics.html)

### Compilation

- [C/C++11 mappings to processors](https://www.cl.cam.ac.uk/~pes20/cpp/cpp0xmappings.html)
- [The JSR-133 Cookbook for Compiler Writers](http://gee.cs.oswego.edu/dl/jmm/cookbook.html)
- https://godbolt.org/

### Cookbook

- [Boost Atomics](https://www.boost.org/doc/libs/1_59_0/doc/html/atomic/usage_examples.html)
- [Using weakly ordered C++ atomics correctly](https://www.youtube.com/watch?v=M15UKpNlpeM) by Hans Boehm
- [A Relaxed Guide to memory_order_relaxed](https://wg21.link/p2055r0) by Paul McKenney & Hans Boehm

----

## Hardware

- [WikiChip](https://en.wikichip.org/wiki/WikiChip)
- [Moore on Moore](https://thechipletter.substack.com/p/moore-on-moore)
- [The Free Lunch Is Over: A Fundamental Turn Toward Concurrency in Software](http://www.gotw.ca/publications/concurrency-ddj.htm)
  by Herb Sutter

### Latency

* [Latency Numbers Every Programmer Should Know](https://gist.github.com/jboner/2841832)
* [Beer cache hierarchy](https://twitter.com/holly_cummins/status/530372145025908737)
* [Measuring CPU core-to-core latency](https://github.com/nviennot/core-to-core-latency)
* [Evaluating the Cost of Atomic Operations on Modern Architectures](https://spcl.inf.ethz.ch/Publications/.pdf/atomic-bench.pdf)

## Threads

### TLS

- [All about thread-local storage](https://maskray.me/blog/2021-02-14-all-about-thread-local-storage)
- [A Deep dive into (implicit) Thread Local Storage](https://chao-tic.github.io/blog/2018/12/25/tls)

## Futures

- [Scala] [Your Server as a Function](https://monkey.org/~marius/funsrv.pdf), [Systems Programming at Twitter](https://monkey.org/~marius/talks/twittersystems/#1), [Futures aren't Ersatz Threads](https://monkey.org/~marius/futures-arent-ersatz-threads.html)
  by Marius
  Eriksen, [Finagle – Concurrent Programming with Futures](https://twitter.github.io/finagle/guide/Futures.html)
    - [C++] [Futures for C++11 at Facebook](https://engineering.fb.com/developer-tools/futures-for-c-11-at-facebook/), [Folly Futures](https://github.com/facebook/folly/blob/master/folly/docs/Futures.md),
- [Rust] [Zero-cost futures in Rust](http://aturon.github.io/blog/2016/08/11/futures/)
  and [Designing futures for Rust](http://aturon.github.io/blog/2016/09/07/futures-design/) by Aaron
  Turon, [RFC](https://github.com/rust-lang/rfcs/blob/master/text/2592-futures.md)
- [C++] [`std::execution`](https://www.open-std.org/jtc1/sc22/wg21/docs/papers/2023/p2300r7.html) by Eric Niebler et al.

## Fibers

- [Fibers, Oh My!](https://graphitemaster.github.io/fibers/) by Dale Weiler
- [Why User-Mode Threads Are Good for Performance](https://www.youtube.com/watch?v=07V08SB1l8c) by Ron Pressler
- [Project Loom: Fibers and Continuations for the Java Virtual Machine](https://cr.openjdk.java.net/~rpressler/loom/Loom-Proposal.html)
  by Ron Pressler
    - [State of Loom, Part 1](http://cr.openjdk.java.net/~rpressler/loom/loom/sol1_part1.html), [Part 2](http://cr.openjdk.java.net/~rpressler/loom/loom/sol1_part2.html)

## Stacks & Continuations

- [Segmented Stacks in LLVM](https://llvm.org/docs/SegmentedStacks.html)
- [Go] [How Stacks are Handled in Go](https://blog.cloudflare.com/how-stacks-are-handled-in-go/), [Continious Stacks Design Doc](https://docs.google.com/document/d/1wAaf1rYoM4S4gtnPh0zOlGzWtrZFQ5suE8qr2sD8uWQ/pub)
- [JVM] [Continuations – Under the Covers](https://www.youtube.com/watch?v=6nRS6UiN7X0) by Ron Pressler
- [Rust] [Abandoning segmented stacks in Rust](https://web.archive.org/web/20150214014326/https://mail.mozilla.org/pipermail/rust-dev/2013-November/006314.html)
- [Rust] [Futures and Segmented Stacks](https://without.boats/blog/futures-and-segmented-stacks/)
- [From Folklore to Fact: Comparing Implementations of Stacks and Continuations](https://people.cs.uchicago.edu/~jhr/papers/2020/pldi-stacks.pdf)

## Coroutines

### Stackless

- By [Russ Cox](https://swtch.com/~rsc/)
    - [On Duff's Device and Coroutines](https://research.swtch.com/duff)
    - [Storing Data in Control Flow](https://research.swtch.com/pcdata)

#### C#

- [How Async/Await Really Works in C#](https://devblogs.microsoft.com/dotnet/how-async-await-really-works/) by Stephen
  Toub

#### C++

- [Asymmetric Transfer](https://lewissbaker.github.io/) by Lewis Baker
    - [Coroutine Theory](https://lewissbaker.github.io/2017/09/25/coroutine-theory)
    - [C++ Coroutines: Understanding operator co_await](https://lewissbaker.github.io/2017/11/17/understanding-operator-co-await)
    - [C++ Coroutines: Understanding the promise type](https://lewissbaker.github.io/2018/09/05/understanding-the-promise-type)
    - [C++ Coroutines: Understanding Symmetric Transfer](https://lewissbaker.github.io/2020/05/11/understanding_symmetric_transfer)
    - [C++ Coroutines: Understanding the Compiler Transform](https://lewissbaker.github.io/2022/08/27/understanding-the-compiler-transform)

#### Kotlin

- [Kotlin Coroutines Design Document](https://github.com/Kotlin/KEEP/blob/master/proposals/coroutines.md)
- [Kotlin coroutines: design and implementation](https://dl.acm.org/doi/abs/10.1145/3486607.3486751)

#### Python

- [A Curious Course on Coroutines and Concurrency](http://www.dabeaz.com/coroutines/)
  by [David Beazley](http://www.dabeaz.com/)

#### Syntax

- [What Color is Your Function?](https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/) by Bob
  Nystrom, [Hacker news](https://news.ycombinator.com/item?id=8984648)
- [C#] [Asynchrony in C# 5 Part Six: Whither async?](https://docs.microsoft.com/en-us/archive/blogs/ericlippert/asynchrony-in-c-5-part-six-whither-async)
  by Eric Lippert
- [Rust] [A final proposal for await syntax](https://boats.gitlab.io/blog/post/await-decision/)
- [Kotlin] [How do you color your functions?](https://medium.com/@elizarov/how-do-you-color-your-functions-a6bb423d936d)
  by Roman Elizarov

## Cancellation & Structured Concurrency

- [Asynchronous Exceptions in Haskell](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/07/asynch-exns.pdf)
  by Simon Marlow, SPJ et al
- [Making Reliable Distributed Systems in the Presence of Software Errors](https://erlang.org/download/armstrong_thesis_2003.pdf)
  by Joe Armstrong
- [Structured Concurrency](https://250bpm.com/blog:71/) by Martin Sústrik
- [Timeouts and cancellation for humans](https://vorpus.org/blog/timeouts-and-cancellation-for-humans/#cancel-scopes-trio-s-human-friendly-solution-for-timeouts-and-cancellation), [Notes on structured concurrency, or: Go statement considered harmful](https://vorpus.org/blog/notes-on-structured-concurrency-or-go-statement-considered-harmful/)
  by Nathaniel J. Smith
- [Structured concurrency](https://medium.com/@elizarov/structured-concurrency-722d765aa952) by Roman Elizarov

### Structured programming

- [Control Structures](https://xavierleroy.org/CdF/2023-2024/) by Xavier Leroy
- [Go To Statement Considered Harmful](https://dl.acm.org/doi/10.1145/362929.362947), [Notes on Structured Programming](https://www.cs.utexas.edu/~EWD/ewd02xx/EWD249.PDF)
  by Edsger Dijkstra
- [The Forgotten Art of Structured Programming](https://www.youtube.com/watch?v=SFv8Wm2HdNM) by Kevlin Henney
- https://xkcd.com/292/

## Scheduling

### Work-stealing

- [Scheduling Multithreaded Computations by Work Stealing](https://dl.acm.org/doi/pdf/10.1145/324133.324234)
- [Go scheduler: Implementing language with lightweight concurrency](https://www.youtube.com/watch?v=-K11rY57K7k) by
  Dmitry Vyukov
- [Making the Tokio scheduler 10x faster](https://tokio.rs/blog/2019-10-scheduler/)
- [Taskflow: A Lightweight Parallel and Heterogeneous Task Graph Computing System](https://taskflow.github.io/papers/tpds21-taskflow.pdf)

#### Implementations

- [Golang] [Runtime](https://golang.org/src/runtime/proc.go)
- [Rust] [Tokio Multi-Threaded Runtime](https://github.com/tokio-rs/tokio/blob/master/tokio/src/runtime/scheduler/multi_thread/worker.rs)
- [Kotlin] [Coroutine Runtime](https://github.com/Kotlin/kotlinx.coroutines/blob/master/kotlinx-coroutines-core/jvm/src/scheduling/CoroutineScheduler.kt)
- [Scala] [Cats Effect](https://github.com/typelevel/cats-effect/blob/series/3.x/core/jvm-native/src/main/scala/cats/effect/unsafe/WorkStealingThreadPool.scala)

#### Queues

##### Chase-Lev

- [Dynamic Circular Work-Stealing Deque](https://www.dre.vanderbilt.edu/~schmidt/PDF/work-stealing-dequeue.pdf) by
  Chase & Lev
- [Correct and Efficient Work-Stealing for Weak Memory Models](https://fzn.fr/readings/ppopp13.pdf)
    - Fence Cumulativity: [Repairing Sequential Consistency in C/C++11](https://plv.mpi-sws.org/scfix/paper.pdf)
- https://github.com/taskflow/work-stealing-queue

##### BWoS

- [BWoS: Formally Verified Block-based Work Stealing for Parallel Processing](https://www.usenix.org/system/files/osdi23-wang-jiawei.pdf)

## Thread-per-core

- [Shared-nothing Design](https://seastar.io/shared-nothing/), https://github.com/scylladb/seastar
- [Introducing Glommio, a Thread-per-Core Crate for Rust & Linux](https://www.datadoghq.com/blog/engineering/introducing-glommio/), https://github.com/DataDog/glommio

---

## Locking

### Spinlocks

- [Correctly implementing a spinlock in C++](https://rigtorp.se/spinlock/) by Erik Rigtorp
- [A Close Look at a Spinlock](https://blog.regehr.org/archives/2173) by John Regehr
- [Locking, from traditional to modern](https://www.youtube.com/watch?v=FsXB-tKUeUw&list=PLVe-2wcL84b9G9o7KPubp6NO0nqI1a-Qp&index=1) ([playlist](https://www.youtube.com/playlist?list=PLVe-2wcL84b9G9o7KPubp6NO0nqI1a-Qp))
  by Nir Shavit

#### Linus Torvalds

- [User-space spinlocks](https://www.realworldtech.com/forum/?threadid=189711&curpostid=189723)
- [sched_yield](https://www.realworldtech.com/forum/?threadid=189711&curpostid=189752)

## Lock-freedom

- [Wait-Free Synchronization](https://cs.brown.edu/~mph/Herlihy91/p124-herlihy.pdf) by Maurice Herlihy, 1991
- [The Art of Multiprocessor Programming](https://shop.elsevier.com/books/the-art-of-multiprocessor-programming/herlihy/978-0-12-415950-1) (
  chapters _The Relative Power of Synchronization Methods_ & _The Universality of Consensus_) by Maurice Herlihy, 2020 (
  2nd edition)
- [On the nature of progress](https://dspace.mit.edu/handle/1721.1/73900) by Maurice Herlihy & Nir Shavit, 2011
- [Are Lock-Free Concurrent Algorithms Practically Wait-Free?](https://arxiv.org/abs/1311.3200) by Dan Alistarh et al,
  2013

### ~ Distributed Consensus

- [Impossibility of Distributed Consensus with One Faulty Process](https://groups.csail.mit.edu/tds/papers/Lynch/jacm85.pdf) –
  FLP theorem – by Fischer, Lynch, Patterson, 1985
- [Unreliable Failure Detectors for Reliable Distributed Systems](https://citeseerx.ist.psu.edu/doc/10.1.1.113.498) by
  Tushar Deepak Chandra and Sam Toueg, 1996

### ~ Linearizability

- [Linearizability: A Correctness Condition for Concurrent Objects](https://cs.brown.edu/~mph/HerlihyW90/p463-herlihy.pdf)
  by Maurice Herlihy and Jannette Wing, 1990
- [Reading the Herlihy & Wing Linearizability paper with TLA<sup>+</sup>](https://github.com/lorin/tla-linearizability),
  📐 [TLA<sup>+</sup> Spec](https://github.com/lorin/tla-linearizability/blob/master/Linearizability.tla) by Lorin
  Hochstein & [Markus Alexander Kuppe](https://github.com/lemmy)

### Data Structures / Algorithms

- [Simple, Fast, and Practical Non-Blocking and Blocking Concurrent Queue Algorithms](https://www.cs.rochester.edu/~scott/papers/1996_PODC_queues.pdf)
- [A Scalable Lock-free Stack Algorithm](https://people.csail.mit.edu/shanir/publications/Lock_Free.pdf)
- [The Baskets Queue](https://people.csail.mit.edu/shanir/publications/Baskets%20Queue.pdf)
- [Lock-free deques and doubly linked lists](http://www.cse.chalmers.se/~tsigas/papers/Lock-Free-Deques-Doubly-Lists-JPDC.pdf)
- [Fast Concurrent Queues for x86 Processors](http://www.cs.tau.ac.il/~mad/mass/17b/lcrq.pdf)
- [moodycamel::ConcurrentQueue](https://github.com/cameron314/concurrentqueue)
    - [Detailed Design of a Lock-Free Queue](https://moodycamel.com/blog/2014/detailed-design-of-a-lock-free-queue.htm)
- [Lock-free структуры данных](https://habr.com/en/post/195770/)

#### Channels

- [Go channels on steroids](https://docs.google.com/document/d/1yIAYmbvL3JxOKOjuCyon7JhW4cSv1wy5hC0ApeGMV9s/pub) by
  Dmitry Vyukov
- [Scalable FIFO Channels for Programming via Communicating Sequential Processes](https://nkoval.com/publications/europar19-channels.pdf)
- [Fast and Scalable Channels in Kotlin Coroutines](https://arxiv.org/pdf/2211.04986.pdf) by Koval, Alistarh, Elizarov,
  2022

### Memory Management

#### Reference counting

- [Differential Reference Counting](http://www.1024cores.net/home/lock-free-algorithms/object-life-time-management/differential-reference-counting)
  by Dmitry Vyukov

#### Atomic shared ptr

##### Talks

- [Implementing a Lock-free
  `atomic_shared_ptr`](https://github.com/brycelelbach/cppnow_presentations_2016/blob/master/01_wednesday/implementing_a_lock_free_atomic_shared_ptr.pdf)
- [A Lock-Free Atomic Shared Pointer in Modern Cpp](https://www.youtube.com/watch?v=gTpubZ8N0no)
- [Lock-free Atomic Shared Pointers Without a Split Reference Count? It Can Be Done!](https://www.youtube.com/watch?v=lNPZV9Iqo3U)

##### Implementations

- [`AtomicSharedPtr`](https://github.com/facebook/folly/blob/main/folly/concurrency/AtomicSharedPtr.h)
  from [Folly](https://github.com/facebook/folly)
- [`TAtomicPtr`](https://github.com/ytsaurus/ytsaurus/blob/main/yt/yt/core/misc/atomic_ptr.h)
  from [YTsaurus](https://github.com/ytsaurus/ytsaurus)
- [`atomic_shared_ptr`](https://github.com/anthonywilliams/atomic_shared_ptr) by Anthony Williams

#### Hazard Pointers

- [Hazard Pointers: Safe Memory Reclamation for Lock-Free Objects](https://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.395.378&rep=rep1&type=pdf)
  by Maged M. Michael

#### Epochs

- [Epoch-Based Memory Reclamation](http://csng.cs.toronto.edu/publication_files/0000/0159/jpdc07.pdf)
- [Lock-freedom without garbage collection](https://aturon.github.io/blog/2015/08/27/epoch/) by Aaron Turon

#### RCU (Read-Copy-Update)

---

## Multi-Word CAS

### Algorithms

- [HFP, 2002] [A Practical Multi-Word Compare-and-Swap Operation](https://www.cl.cam.ac.uk/research/srg/netos/papers/2002-casn.pdf)
- [GKMZ, 2020] [Efficient Multi-word Compare and Swap](https://arxiv.org/abs/2008.02527)

### Implementations

- [Kcas — STM based on lock-free MCAS](https://github.com/ocaml-multicore/kcas)
    - [Kcas: Building a Lock-Free STM for OCaml](https://tarides.com/blog/2023-08-07-kcas-building-a-lock-free-stm-for-ocaml-1-2/)
    - [k-CAS for sweat-free concurrent programming](https://gist.github.com/polytypic/3214389ad69b16d28b957ced86e1b1a4#k-cas-for-sweat-free-concurrent-programming)
    - [Extending k-CAS with efficient read-only CMP operations](https://gist.github.com/polytypic/0efa0e2981d2a5fc4b534a0e25120cc9)

### Reagents

by Aaron Turon

- [Reagents: Expressing and Composing Fine-grained Concurrency](https://www.khoury.northeastern.edu/home/turon/reagents.pdf)
- [Slides](https://aturon.github.io/academic/pldi-2012-reagents.pdf)
- [Thesis](https://www.khoury.northeastern.edu/home/turon/thesis.pdf)

## Transactions

### HTM

- [Transactional Memory: Architectural Support for Lock-Free Data Structures](https://cs.brown.edu/~mph/HerlihyM93/herlihy93transactional.pdf)
  by Maurice Herlihy
- [Talk](https://www.youtube.com/watch?v=ZkUrl8BZHjk), [Slides](http://neerc.ifmo.ru/sptcc/slides/slides-herlihy.pdf) by
  Maurice Herlihy
- [Understanding Hardware Transactional Memory](https://www.youtube.com/watch?v=0jy4Sc_IY7o) by Gile Tene
- [The Azul Hardware Transactional Memory Experience](https://www.youtube.com/watch?v=GEkeOHw87Sg) by Cliff Click
- [Is Parallel Programming Hard, And, If So, What Can You Do About It?](https://mirrors.edge.kernel.org/pub/linux/kernel/people/paulmck/perfbook/perfbook.html),
  17.3 by Paul McKenney
- [Глава 16 – Programming with Intel Transactional Synchronization Extensions](https://software.intel.com/sites/default/files/managed/39/c5/325462-sdm-vol-1-2abcd-3abcd.pdf)
- [Глава 15 – Intel TSX Recommendations](https://software.intel.com/sites/default/files/managed/9e/bc/64-ia-32-architectures-optimization-manual.pdf)
- [TSX Anti-Patterns](https://software.intel.com/en-us/articles/tsx-anti-patterns-in-lock-elision-code)
- [Lock Elision Implementation Guide](https://sourceware.org/glibc/wiki/LockElisionGuide)

### STM

- [Haskell] [Composable Memory Transactions](https://cs.brown.edu/people/mph/HarrisMPJH05/stm.pdf) by Harris, Marlow,
  Peyton Jones, Herlihy, 2005
- [A (brief) retrospective on transactional memory](https://joeduffyblog.com/2010/01/03/a-brief-retrospective-on-transactional-memory/)
  by Joe Duffy
- [Scala, ZIO] [Introduction to Software Transactional Memory](https://zio.dev/reference/stm/)
- [Multicore OCaml] [Kcas — STM based on lock-free MCAS](https://github.com/ocaml-multicore/kcas)
- [Transactional Locking II](http://people.csail.mit.edu/shanir/publications/Transactional_Locking.pdf)

## IO

### Model

#### Readiness

- [epoll]()
- [kqueue]()

#### Completeness

- [Windows IOCP]()
- [Efficient IO with io_uring](https://kernel.dk/io_uring.pdf)
- [Lord of the io_uring](https://unixism.net/loti/)

### API

- [A Universal I/O Abstraction for C++](https://cor3ntin.github.io/posts/iouring/#reactors-select-poll-epoll)

## ~ FP

### Monads

- [Monads for functional programming](https://homepages.inf.ed.ac.uk/wadler/papers/marktoberdorf/baastad.pdf)
- [Imperative functional programming](https://www.microsoft.com/en-us/research/wp-content/uploads/1993/01/imperative.pdf)
- [Typeclassopedia](https://wiki.haskell.org/Typeclassopedia)
- [A monad is just a monoid in the category of endofunctors, what's the problem?](https://stackoverflow.com/questions/3870088/a-monad-is-just-a-monoid-in-the-category-of-endofunctors-whats-the-problem)
- [Abstraction, intuition, and the “monad tutorial fallacy”](https://byorgey.wordpress.com/2009/01/12/abstraction-intuition-and-the-monad-tutorial-fallacy/)

### Monadic Concurrency

- [Concurrent Haskell](https://www.microsoft.com/en-us/research/wp-content/uploads/1996/01/concurrent-haskell.pdf) by
  SPJ et al
- [A Monad for Deterministic Parallelism](https://simon.peytonjones.org/assets/pdfs/monad-par.pdf) by Simon Marlow, Ryan
  Newton & SPJ
- [Composable Memory Transactions](https://timharris.uk/papers/2005-ppopp-composable.pdf) by Tim Harris, Simon Marlow,
  SPJ & Maurice Herlihy
- [Reagents: Expressing and Composing Fine-grained Concurrency](https://www.khoury.northeastern.edu/home/turon/reagents.pdf)
  by Aaron Turon

## Formal Languages

- [Communicating Sequential Processes](http://www.usingcsp.com/cspbook.pdf) (CSP) by C. A. R. Hoare
- [A Science of Concurrent Programs](https://lamport.azurewebsites.net/tla/science.pdf) (TLA) by Leslie Lamport

## Verification

### Race detection

- [ThreadSanitizer – data race detection in practice](https://static.googleusercontent.com/media/research.google.com/en//pubs/archive/35604.pdf), [compiler-rt/lib/tsan/rtl](https://github.com/llvm-mirror/compiler-rt/tree/master/lib/tsan/rtl)
- [FastTrack: Efficient and Precise Dynamic Race Detection](https://users.soe.ucsc.edu/~cormac/papers/pldi09.pdf)

### PCT

- [A Randomized Scheduler with Probabilistic Guarantees of Finding Bugs](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/asplos277-pct.pdf)

### DPOR

- [Partial-Order Methods for the Verification of Concurrent Systems: An Approach to the State-Explosion Problem](https://patricegodefroid.github.io/public_psfiles/thesis.pdf)
  by Patrice Godefroid, 1996
- [SeqCst] [Dynamic Partial-Order Reduction for Model Checking Software](https://users.soe.ucsc.edu/~cormac/papers/popl05.pdf)
  by Flanagan & Godefroid, 2005
    - [Fair Stateless Model Checking](https://www.microsoft.com/en-us/research/publication/fair-stateless-model-checking/)
      by Musuvathi & Qadeer, 2008
    - [Bounded Partial-Order Reduction](https://www.microsoft.com/en-us/research/publication/bounded-partial-order-reduction/)
      by Musuvathi et al, 2013
- [C++11] [A Practical Approach for Model Checking C/C++11 Code](http://plrg.eecs.uci.edu/publications/toplas16.pdf) by
  Norris & Demsky, 2016
- [SeqCst] [Source Sets: A Foundation for Optimal Dynamic Partial Order Reduction](https://user.it.uu.se/~parosh/publications/papers/jacm17.pdf)
  by Abdulla et al, 2017
    - [Comparing Source Sets and Persistent Sets for Partial Order Reduction](https://user.it.uu.se/~bengt/Papers/Full/kimfest17.pdf)
    - [Optimal Dynamic Partial Order Reduction with Observers](https://user.it.uu.se/~bengt/Papers/Full/tacas18.pdf)
- [RC11] [Truly Stateless, Optimal Dynamic Partial Order Reduction](https://plv.mpi-sws.org/genmc/popl2022-trust.pdf) by
  Kokologiannakis et al, 2022

