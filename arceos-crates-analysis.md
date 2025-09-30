# ArceOS 内核功能Crates 总览

## 1. 引言

- 简述ArceOS：基于Rust的组件化Unikernel内核
- 内核组件化思路：每个crate是一个功能模块，按需组成最小内核
- 文档目标：分析modules/下的核心crate，并且点名其它外围crate的定位与边界。

## 2. Crate总览

- modules/ 内核核心功能
- api/ 对外接口层
- ulib/ 面向应用的运行时与库（非内核）


## 3.内核核心Crate (modules/)

### 3.1 axhal
- 功能定位：
硬件抽象层，封装各平台引导、陷入/中断、时间、内存、CPU/多核、TLS 等原语，为上层提供统一接口。屏蔽体系结构和平台差异。平台由axhal完成早期硬件初始化与抽象层搭建，通过建立栈，启动MMU分页机制，重置栈指针，再调用axruntime::rust_main(...)进入上层运行时编排，上层随后通过axhal具备访问硬件的能力。

lib.rs
```
//! [ArceOS] hardware abstraction layer, provides unified APIs for
//! platform-specific operations.
//!
//! It does the bootstrapping and initialization process for the specified
//! platform, and provides useful operations on the hardware.
//!
```

- 组成与对外接口：
-- platform/：平台选择与具体实现（console、mp、init 等）
trap：陷入/异常框架（向量表、现场保存与分发）
arch：与 ISA 相关的内联汇编与寄存器操作（开关中断、等待中断、线程指针等）
cpu：CPU 相关（获取当前 CPU ID 等）
mem：物理内存区域枚举、地址转换（phys_to_virt/virt_to_phys）
time：单调时钟/墙钟时间、一次性定时器、定时器 IRQ 号
irq：中断注册/启用（按 feature）
paging：页表操作接口（按 feature）
tls：线程本地存储（按 feature）
misc：终止系统等杂项


lib.rs
```
mod platform;

#[macro_use]
pub mod trap;

pub mod arch;
pub mod cpu;
pub mod mem;
pub mod time;

#[cfg(feature = "tls")]
pub mod tls;

#[cfg(feature = "irq")]
pub mod irq;

#[cfg(feature = "paging")]
pub mod paging;
```

- 在内核各子系统中的核心职责
-- 日志与输出：

为 axlog 提供底层的字符输出能力。上层 axruntime 通过调用 axhal::console::write_bytes 实现控制台日志的输出。

-- 时间与定时器：

提供 monotonic_time/wall_time_nanos 获取当前时间。

提供 set_oneshot_timer 设置一次性定时器和 TIMER_IRQ_NUM。这些功能被 axruntime 的定时器中断初始化机制使用，用于驱动系统 调度时钟。

-- CPU 与 SMP：

cpu::this_cpu_id() 提供当前运行核的标识符。

mp::start_secondary_cpu 用于 次核的引导，随后 platform_init_secondary 完成次核的平台层初始化。

-- 内存与地址空间：

mem::memory_regions() 暴露底层 物理内存的分布信息。

phys_to_virt/virt_to_phys 用于全局分配器（如堆分配器）的落地和地址映射协作。

paging 子模块（若启用）提供页表相关的 API。

-- 中断/陷入：

trap 模块负责 保存/恢复现场 并将异常/中断事件进行分发。

irq 模块提供中断向量的注册和中断的启用/禁用。

arch::enable_irqs() 和 arch::wait_for_irqs() 控制中断的使能状态和进入低功耗的等待模式。

-- TLS 与线程指针：

在没有多任务的场景下，由 axruntime 触发，通过 tls::TlsArea::alloc() 分配 TLS 区域，并通过 arch::write_thread_pointer(...) 为主线程设置线程本地存储指针。

平台抽象与关机：

platform_init() 完成平台级的设备和控制器初始化。

misc::terminate() 用于处理系统 panic 或正常退出时的 关机路径。

- 对上层的价值：
让上层的内核的crate(axruntime, axtask,axmm,axdriver等)，面对统一的API，而将平台差异封装在下。上层内核经由这些统一接口完成初始化、调度与设备子系统装配，从而实现“同一内核，多平台运行。

### 3.2 axruntime
- 功能定位：运行时框架，负责内核引导，初始化和任务启动。它在平台引导完成后接管控制，按 feature 决定性地初始化各内核子系统（分配器、分页、驱动、FS、网络、显示、调度器、中断、TLS、SMP），桥接日志上下文，最后转入应用的 main() 或 idle 循环。
是ArceOS的运行时库，提供系统的主入口点并协调系统初始化，然后将控制权转移给用户应用程序。

- 主初始化流程：

主要入口点是rust_main()函数，它由平台特定的引导代码调用，处理完整的系统初始化序列。

lib.rs
```
pub extern "C" fn rust_main(cpu_id: usize, dtb: usize) -> ! {
    ax_println!("{}", LOGO);
    ax_println!("arch = {}\nplatform = {}\ntarget = {}\nsmp = {}\nbuild_mode = {}\nlog_level = {}\n",
        option_env!("AX_ARCH").unwrap_or(""),
        option_env!("AX_PLATFORM").unwrap_or(""),
        option_env!("AX_TARGET").unwrap_or(""),
        option_env!("AX_SMP").unwrap_or(""),
        option_env!("AX_MODE").unwrap_or(""),
        option_env!("AX_LOG").unwrap_or(""),
    );
    axlog::init();
    axlog::set_max_level(option_env!("AX_LOG").unwrap_or(""));
    info!("Primary CPU {} started, dtb = {:#x}.", cpu_id, dtb);
```
lib.rs
```
#[cfg(any(feature = "alloc", feature = "alt_alloc"))] init_allocator();
#[cfg(feature = "paging")] axmm::init_memory_management();
info!("Initialize platform devices..."); axhal::platform_init();
#[cfg(feature = "multitask")] axtask::init_scheduler();
{
    #[allow(unused_variables)]
    let all_devices = axdriver::init_drivers();
    #[cfg(feature = "fs")] axfs::init_filesystems(all_devices.block);
    #[cfg(feature = "net")] axnet::init_network(all_devices.net);
    #[cfg(feature = "display")] axdisplay::init_display(all_devices.display);
}
#[cfg(feature = "smp")] self::mp::start_secondary_cpus(cpu_id);
#[cfg(feature = "irq")] { info!("Initialize interrupt handlers..."); init_interrupt(); }
```

lib.rs
```
INITED_CPUS.fetch_add(1, Ordering::Relaxed);
while !is_init_ok() { core::hint::spin_loop(); }
unsafe { main() };
#[cfg(feature = "multitask")] axtask::exit(0);
#[cfg(not(feature = "multitask"))] { debug!("main task exited: exit_code={}", 0); axhal::misc::terminate(); }
```

初始化遵循着下面顺序：
1) 打印 banner/构建信息 → 2) axlog::init() 与 loglevel → 3) 列出物理内存区域 → 4) 分配器初始化（选择最大空闲区域为主堆，其余追加） → 5) axmm::init_memory_management()（如启用分页） → 6) axhal::platform_init() 平台/设备 → 7) axtask::init_scheduler()（多任务） → 8) axdriver::init_drivers() 并据 fs/net/display 装配 → 9) mp::start_secondary_cpus()（SMP） → 10) init_interrupt() 注册定时器 IRQ 并开启中断（irq） → 11) init_tls()（tls 且非多任务） → 12) 等待所有 CPU 完成初始化 → 13) 最后调用用户的 main() → 14) 正常退出路径。

- 多核支持
对于SMP系统，axruntime还提供了rust_main_secondary()用于辅助CPU初始化，最终进入任务调度或空闲循环
mp.rs
```
/// The main entry point of the ArceOS runtime for secondary cores.
///
/// It is called from the bootstrapping code in the specific platform crate.
#[axplat::secondary_main]
pub fn rust_main_secondary(cpu_id: usize) -> ! {
    axhal::init_percpu_secondary(cpu_id);
    axhal::init_early_secondary(cpu_id);
```

- 与ArceOS内核的集成
axruntime是ArceOS应用程序的必须依赖项，它与axhal的契合点构成了平台屏障，axruntime，通过 axfeat 特性编排系统进行集成。而且它作为其它模块（如axhal，axtask和axfs）依赖的基础，确保正确的系统初始化。

### 3.3 axalloc
- 功能定位：
动态内存分配框架（实现了字节分配器和页分配器），实现了Rust标准库的GlobalAlloc trait，为整个系统提供动态内存分配功能

- 核心架构
axalloc采用两级分配器设计：
-- 字节分配器：处理小块内存分配，支持TLSF、Slab和Buddy三种算法

lib.rs
```
cfg_if::cfg_if! {
    if #[cfg(feature = "slab")] {
        /// The default byte allocator.
        pub type DefaultByteAllocator = allocator::SlabByteAllocator;
    } else if #[cfg(feature = "buddy")] {
        /// The default byte allocator.
        pub type DefaultByteAllocator = allocator::BuddyByteAllocator;
    } else if #[cfg(feature = "tlsf")] {
        /// The default byte allocator.
        pub type DefaultByteAllocator = allocator::TlsfByteAllocator;
    }
}
```

-- 页分配器：管理大块页面内存，使用位图页分配器
lib.rs
```
pub struct GlobalAllocator {
    balloc: SpinNoIrq<DefaultByteAllocator>,
    palloc: SpinNoIrq<BitmapPageAllocator<PAGE_SIZE>>,
}
```

- 关键工作流程
-- 初始化阶段 （由axruntime调用）
1) axruntime 从 axhal::mem::memory_regions() 挑选最大空闲物理段，做线性映射 phys_to_virt，调用 global_init(start, size) 作为首段堆。
2) 其余空闲段逐一 global_add_memory(...) 作为追加堆。
3) GlobalAllocator::init 逻辑：
先把整段加入页分配器（bitmap）。
再从页分配器“借”出 32KB，初始化字节分配器的启动堆。

lib.rs
```
fn init_allocator() {
    // 选择最大空闲段为起始堆
    axalloc::global_init(phys_to_virt(r.paddr).as_usize(), r.size);
    // 其余空闲段作为追加堆
    axalloc::global_add_memory(phys_to_virt(r.paddr).as_usize(), r.size)?;
}
```

lib.rs
```
pub fn init(&self, start_vaddr: usize, size: usize) {
    assert!(size > MIN_HEAP_SIZE);
    self.palloc.lock().init(start_vaddr, size);
    let heap_ptr = self.alloc_pages(init_heap_size / PAGE_SIZE, PAGE_SIZE).unwrap();
    self.balloc.lock().init(heap_ptr, init_heap_size);
}
```

-- 运行时分配路径（字节级）
先尝试从字节分配器分配；若失败，则：
1) 计算扩容大小 expand_size = max(old_heap, req).next_power_of_two().max(4K)。
2) 从页分配器 alloc_pages(expand_size/4K, 4K) 拿一段。
3) 将该段“加回”字节分配器的堆池，再重试。
释放时直接回收给字节分配器。
lib.rs
```
pub fn alloc(&self, layout: Layout) -> AllocResult<NonNull<u8>> {
    let mut balloc = self.balloc.lock();
    loop {
        if let Ok(ptr) = balloc.alloc(layout) { return Ok(ptr); }
        else {
            let old_size = balloc.total_bytes();
            let expand_size = old_size.max(layout.size()).next_power_of_two().max(PAGE_SIZE);
            let heap_ptr = self.alloc_pages(expand_size / PAGE_SIZE, PAGE_SIZE)?;
            balloc.add_memory(heap_ptr, expand_size)?;
        }
    }
}
```



-- 运行时分配路径（页级）
直接使用页分配器 alloc_pages(num_pages, align_pow2)；释放用 dealloc_pages。
典型用例通过 RAII 封装 GlobalPage 管理生命周期。

lib.rs
```
pub fn alloc_pages(&self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
    self.palloc.lock().alloc_pages(num_pages, align_pow2)
}
pub fn dealloc_pages(&self, pos: usize, num_pages: usize) {
    self.palloc.lock().dealloc_pages(pos, num_pages)
}
```

- 与alt_axalloc crate的区别：
简明概况一下，axalloc 是“功能完整、可扩容”的两级分配器；alt_axalloc 是“简化/早期版”的单体分配器（EarlyAllocator），不支持动态扩容，适合极简/早期阶段。

alt_axalloc: 基于 bump_allocator::EarlyAllocator<PAGE_SIZE> 的单体实现，接口兼容但能力更简单。

关于接口的话，axalloc.name() 返回 slab/buddy/TLSF；alt_axalloc.name() 固定 early。

默认选用axalloc即可。

### 3.4 axmm

- 功能定位：
axmm是ArceOS里的虚拟内存管理模块，负责管理内核和用户进程的地址空间，提供了页表操作还有内存区域映射的功能。对上层暴露统一的地址空间抽象 AddrSpace，对下借助 axhal::paging 和物理内存信息完成具体页表操作。

- 核心对外能力：
-- 地址空间创建：
new_kernel_aspace()：构建内核地址空间（线性映射物理内存区域）。
new_user_aspace()：构建用户地址空间（复制必要内核映射）。
-- 内核全局地址空间：
kernel_aspace()、kernel_page_table_root()
-- 初始化（主/次核）：
init_memory_management()：建立内核页表并设置为当前内核页表根。
init_memory_management_secondary()：在次核写入同一页表根。

这些接口在lib.rs可以找到
```
pub fn new_kernel_aspace() -> AxResult<AddrSpace> {
    let mut aspace = AddrSpace::new_empty(
        va!(axconfig::KERNEL_ASPACE_BASE),
        axconfig::KERNEL_ASPACE_SIZE,
    )?;
    for r in axhal::mem::memory_regions() {
        aspace.map_linear(phys_to_virt(r.paddr), r.paddr, r.size, r.flags.into())?;
    }
    Ok(aspace)
}
...
pub fn init_memory_management() {
    info!("Initialize virtual memory management...");
    let kernel_aspace = new_kernel_aspace().expect("failed to initialize kernel address space");
    KERNEL_ASPACE.init_once(SpinNoIrq::new(kernel_aspace));
    axhal::paging::set_kernel_page_table_root(kernel_page_table_root());
}
```

- 地址空间操作 (AddrSpace)
axmm 实现了Linear Backend和Allocation Backend两种映射策略。
线性映射后端 (Linear Backend)
用途：用于物理地址连续且已知的内存区域（如设备内存、内核区域）
特点：虚拟地址到物理地址的偏移量固定（pa_va_offset）
优势：无需页错误处理，映射关系简单高效
分配映射后端 (Allocation Backend)
用途：用于动态分配的内存，支持懒分配和即时分配两种模式
即时分配模式 (populate=true)：映射创建时立即分配所有物理页面
懒分配模式 (populate=false)：仅创建页表条目，物理页面在访问时通过页错误分配
内存管理：与 axalloc 全局分配器集成，支持页面级分配和释放

### 3.5 axtask

- 功能定位：
承担内核的多任务调度和管理功能，是ArceOS的任务管理模块，包括任务创建，调度，同步，生命周期管理并且支持协作式和抢占式调度算法。

- 通过Feature，可以选择的调度算法如下：
FIFO（合作式）：默认；不抢占，任务主动让出 CPU（适合简单/可控场景）。
RR（时间片轮转，抢占式）：按固定时间片轮流跑；时间片到了就切换。
CFS（完全公平调度）：尽量让每个任务得到“公平”的 CPU 时间。
（sched_fifo、sched_rr、sched_cfs）


- 工作流程（从开机到任务切换）

1. 初始化调度器
系统启动后，运行时会初始化 axtask 的调度器，准备就绪队列与当前任务指针。
2. 创建首批任务
内核或用户态运行时会创建第一个应用任务（以及必要的 idle 任务），并把它们丢进就绪队列。
3. 选择要运行的任务
调度器从就绪队列里按所选算法挑一个任务，保存/恢复上下文，通过 axhal 进行上下文切换到该任务。
4. 抢占与时间片（如果开启抢占）
时钟中断到来：减少当前任务的剩余时间片；时间片用完或有更合适的任务时，触发调度切换。
这一步由 irq + timer协同完成，由axtask负责排班和切换。
5. 阻塞与唤醒
任务调用 sleep 或在等待队列上阻塞时，会从就绪队列移到等待队列。
条件满足或超时后，被唤醒并重新放回就绪队列，等待再次被调度。
6. 任务退出
任务调用 exit 或返回结束，axtask 回收资源，把 CPU 让给下一个任务。

api.rs里提供了完整的任务管理接口，包括任务创建，让出CPU，睡眠和退出。这些API被上层应用和其它内核模块广泛使用。



### 3.6 axdriver
- 功能定位：
提供了一个统一的设备驱动框架，支持网络、块存储和显示设备的管理。该模块支持静态和动态两种设备模型，并通过 PCI 和 MMIO 总线自动发现设备。axdriver具有两种设备模型，一种是Static(默认)，另一种是Dynamic，以trait的形式存在(需要手动启用)

- 在内核里面的作用:

axruntime 在启动阶段调用 axdriver::init_drivers()，完成设备探测与实例化，返回 AllDevices。
随后将不同类别的设备容器分发给各上层子系统：axfs（文件系统）、axnet（网络栈）、axdisplay（图形显示）。

lib.rs
```
        let all_devices = axdriver::init_drivers();
        ...
        axfs::init_filesystems(all_devices.block);
        ...
        axnet::init_network(all_devices.net);
        ...
        axdisplay::init_display(all_devices.display);
```

lib.rs
```
use axdriver::{prelude::*, AxDeviceContainer};

/// Initializes filesystems by block devices.
pub fn init_filesystems(mut blk_devs: AxDeviceContainer<AxBlockDevice>) {
    info!("Initialize filesystems...");
    let dev = blk_devs.take_one().expect("No block device found!");
    info!("  use block device 0: {:?}", dev.device_name());
    self::root::init_rootfs(self::dev::Disk::new(dev));
}
```

lib.rs
```
use axdriver::{prelude::*, AxDeviceContainer};

/// Initializes the network subsystem by NIC devices.
pub fn init_network(mut net_devs: AxDeviceContainer<AxNetDevice>) {
    info!("Initialize network subsystem...");
    let dev = net_devs.take_one().expect("No NIC device found!");
    info!("  use NIC 0: {:?}", dev.device_name());
    net_impl::init(dev);
}
```

它的内部，通过总线与具体驱动“探测+统一封装”，最终聚合到 AllDevices，供上层消费。这一流程由 init_drivers() 驱动，内部会统计设备并做基本断言与日志输出
lib.rs
```
pub fn init_drivers() -> AllDevices {
    info!("Initialize device drivers...");
    info!("  device model: {}", AllDevices::device_model());

    let mut all_devs = AllDevices::default();
    all_devs.probe();

    #[cfg(feature = "net")]
    {
        debug!("number of NICs: {}", all_devs.net.len());
        for (i, dev) in all_devs.net.iter().enumerate() {
            assert_eq!(dev.device_type(), DeviceType::Net);
            debug!("  NIC {}: {:?}", i, dev.device_name());
        }
    }
    #[cfg(feature = "block")]
    {
        debug!("number of block devices: {}", all_devs.block.len());
        for (i, dev) in all_devs.block.iter().enumerate() {
            assert_eq!(dev.device_type(), DeviceType::Block);
            debug!("  block device {}: {:?}", i, dev.device_name());
        }
    }
    #[cfg(feature = "display")]
    {
        debug!("number of graphics devices: {}", all_devs.display.len());
        for (i, dev) in all_devs.display.iter().enumerate() {
            assert_eq!(dev.device_type(), DeviceType::Display);
            debug!("  graphics device {}: {:?}", i, dev.device_name());
        }
    }

    all_devs
}
```


### 3.7 axfs
- 功能定位：
 ArceOS 的文件系统模块，提供了虚拟文件系统（VFS）层，支持多种文件系统类型，是基于块设备初始化根文件系统。

- 工作流程：

1) 上层调用入口：axruntime 在完成驱动探测后，将块设备容器交给 axfs。

lib.rs
```
let all_devices = axdriver::init_drivers();
...
axfs::init_filesystems(all_devices.block);
```

2) axfs 抽取一个块设备并初始化根文件系统：

lib.rs
```
pub fn init_filesystems(mut blk_devs: AxDeviceContainer<AxBlockDevice>) {
    info!("Initialize filesystems...");
    let dev = blk_devs.take_one().expect("No block device found!");
    info!("  use block device 0: {:?}", dev.device_name());
    self::root::init_rootfs(self::dev::Disk::new(dev));
}
```

3) 构建根目录、选择主 FS、挂载附加 FS，并设置当前目录与根目录句柄：


root.rs
```
pub(crate) fn init_rootfs(disk: crate::dev::Disk) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "myfs")] {
            let main_fs = fs::myfs::new_myfs(disk);
        } else if #[cfg(feature = "fatfs")] {
            static FAT_FS: LazyInit<Arc<fs::fatfs::FatFileSystem>> = LazyInit::new();
            FAT_FS.init_once(Arc::new(fs::fatfs::FatFileSystem::new(disk)));
            FAT_FS.init();
            let main_fs = FAT_FS.clone();
        }
    }
    let mut root_dir = RootDirectory::new(main_fs);

    #[cfg(feature = "devfs")]
    root_dir.mount("/dev", mounts::devfs())?;
    #[cfg(feature = "ramfs")]
    root_dir.mount("/tmp", mounts::ramfs())?;
    #[cfg(feature = "procfs")]
    root_dir.mount("/proc", mounts::procfs().unwrap())?;
    #[cfg(feature = "sysfs")]
    root_dir.mount("/sys", mounts::sysfs().unwrap())?;

    ROOT_DIR.init_once(Arc::new(root_dir));
    CURRENT_DIR.init_once(Mutex::new(ROOT_DIR.clone()));
    *CURRENT_DIR_PATH.lock() = "/".into();
}
```
4) mounts 中具体准备各挂载的实例（例如 devfs/ramfs 等）：
mounts.rs
```
#[cfg(feature = "devfs")]
pub(crate) fn devfs() -> Arc<fs::devfs::DeviceFileSystem> {
    let null = fs::devfs::NullDev;
    let zero = fs::devfs::ZeroDev;
    let bar = fs::devfs::ZeroDev;
    let devfs = fs::devfs::DeviceFileSystem::new();
    let foo_dir = devfs.mkdir("foo");
    devfs.add("null", Arc::new(null));
    devfs.add("zero", Arc::new(zero));
    foo_dir.add("bar", Arc::new(bar));
    Arc::new(devfs)
}
```



### 3.8 axnet
- 功能定位：
axnet为内恶化提供了统一的网络栈入口和API，基于来自axdriver的网卡设备初始化网络，具有网络协议栈集成，对上暴露TCP/UDP 嵌套字（提供了TcpSocket、UdpSocket的API）。提供 dns_query、poll_interfaces，以及 bench_transmit/receive 等基准接口。从 axdriver 获取一个 NIC 设备并完成网络栈初始化。

- 初始化
lib.rs
```
use axdriver::{prelude::*, AxDeviceContainer};

/// Initializes the network subsystem by NIC devices.
pub fn init_network(mut net_devs: AxDeviceContainer<AxNetDevice>) {
    info!("Initialize network subsystem...");
    let dev = net_devs.take_one().expect("No NIC device found!");
    info!("  use NIC 0: {:?}", dev.device_name());
    net_impl::init(dev);
}
```


### 3.9 axlog 

- 功能定位：
提供调试和监控能力，全局可用

- 典型用法：
```
use axlog::{info, debug, error};

// 初始化日志系统
axlog::init();

// 不同级别的日志输出
info!("System initialized");
debug!("Debug information: {}", value);
error!("Error occurred: {}", error_msg);
```

- 主要功能：
-- 多级别日志宏
error!, warn!, info!, debug!, trace! 五级日志
支持编译时级别控制，可通过特性完全禁用低级别日志
-- 灵活的输出格式
标准环境：显示时间戳、CPU ID、任务 ID、文件位置
no_std 环境：通过接口 trait 输出，支持自定义格式
-- 彩色输出支持
ANSI 颜色代码支持，不同日志级别使用不同颜色
可选的时间戳、CPU ID、任务 ID 显示
-- 外部接口支持
LogIf trait 允许外部实现控制台输出、时间获取等功能
支持运行时动态调整日志级别

### 3.10 axsync 

- 功能定位：
提供内核级别的同步机制，确保多任务和多核环境下的数据一致性和互斥访问。

- 主要功能：

-- 互斥锁 (Mutex)
多任务环境：基于自旋锁和阻塞队列的完整互斥锁实现
单任务环境：退化为无中断自旋锁 (SpinNoIrq)
支持递归锁定和守卫模式
-- 自旋锁集成
重导出 kspin 库的自旋锁原语
提供无中断自旋锁 (SpinNoIrq) 等多种自旋锁变体
支持中断禁用和优先级继承
-- 多任务支持
启用 multitask 特性时，提供完整的多线程同步机制
与 axtask 模块集成，支持任务调度和阻塞队列

### 3.11 axconfig

- 功能定位：
提供平台特定的常量和参数，实现硬件抽象层的配置管理。通常在最早初始化，提供平台参数供其他模块使用。

### 3.12 axdma

- 功能定位：

提供了内核级别的 DMA（直接内存访问）内存分配和管理，确保硬件设备能够高效、安全地进行内存直接访问。它有双层分配策略

dma.rs
```
fn alloc_coherent_bytes(&mut self, layout: Layout) -> AllocResult<DMAInfo> {
    // 首先尝试从内部字节分配器分配
    if let Ok(data) = self.alloc.alloc(layout) {
        return Ok(DMAInfo {
            cpu_addr: data,
            bus_addr: virt_to_bus(va!(data.as_ptr() as usize)),
        });
    }
    
    // 分配失败时自动扩展内存池
    let num_pages = 4.min(available_pages);
    let vaddr_raw = global_allocator().alloc_pages(num_pages, PAGE_SIZE_4K)?;
    // 设置不可缓存属性
    self.update_flags(va!(vaddr_raw), num_pages, 
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::UNCACHED)?;
}
···
fn alloc_coherent_pages(&mut self, layout: Layout) -> AllocResult<DMAInfo> {
    let num_pages = layout_pages(&layout);
    // 直接分配连续的物理页面
    let vaddr_raw = global_allocator().alloc_pages(num_pages, 
        PAGE_SIZE_4K.max(layout.align()))?;
    
    // 设置 DMA 专用的内存属性
    self.update_flags(va!(vaddr_raw), num_pages,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::UNCACHED)?;
}

```

上面分别展示了字节级分配和页面级分配

### 3.13 riscv_vcpu

- 功能定位：
这是 ArceOS 的虚拟化扩展模块，专门为 RISC-V 架构提供硬件虚拟化支持，使 ArceOS 能够作为 Hypervisor 运行。

虚拟CPU架构
```
pub struct RISCVVCpu {
    // CPU 寄存器状态
    pub guest_regs: GuestCpuState,      // Guest GPR 和 CSR
    pub vs_csrs: GuestVsCsrs,          // VS-level CSRs (V=1 时有效)
    pub virtual_hs_csrs: GuestVirtualHsCsrs, // 虚拟化的 HS-level CSRs
    
    // 虚拟机控制
    pub vcpu_id: usize,                // vCPU ID
    pub vm_id: usize,                  // 所属 VM ID
    
    // 内存管理
    pub guest_page_table: PhysAddr,    // Guest 页表根地址
    pub guest_memory_regions: Vec<(GuestPhysAddr, usize)>, // Guest 物理内存
}
```

- 硬件虚拟化支持：
它实现了H 扩展检测：检查处理器是否支持 RISC-V 虚拟化扩展
CSR 管理：设置和管理虚拟化相关的控制状态寄存器
异常委派：配置哪些异常应该委派给 Guest OS 处理
详细可见ArceOS训练营第三期Hypervisor课件

- 内存虚拟化：

-- 两阶段地址转换：GVA → GPA → HPA
-- 影子页表：维护 Host 的页表映射 Guest 内存
-- 内存保护：隔离 Guest 和 Host 的内存访问


- VM Exit处理
```
pub enum AxVCpuExitReason {
    // 同步异常
    GuestEcall,           // ECALL from Guest
    GuestLoadPageFault,   // Guest 加载页错误
    GuestStorePageFault,  // Guest 存储页错误
    GuestInstPageFault,   // Guest 指令页错误
    
    // 中断
    GuestTimerInterrupt,  // Guest 定时器中断
    GuestExternalInterrupt, // Guest 外部中断
    
    // 其他
    VmShutdown,          // VM 关闭
    VmReset,            // VM 重置
}
```

使 ArceOS 能够作为虚拟机监控器运行多个 Guest OS，能够支持Hypervisor

## 4.对外接口Crates

### api/

- 功能定位：
api/ 是ArceOS的公共接口层和特性总控层，内包含三个crate,分别是arceos_api，arceos_posix_api，axfeat，负责把“哪些功能启用、对外暴露什么 API、未启用时如何优雅退化”这些问题一次性处理好。

-- 启用路径: 先用 axfeat 选择功能 → arceos_api/arceos_posix_api 按需导出统一接口。

- arceos_api:
作用: 内核模块的公共 API 门面（facade）。对上稳定导出类型与函数，按 feature 把 axfs、axnet、axdisplay 等纳入统一入口。
特点: 与 axfeat 的开关联动；支持未启用时的占位实现（dummy-if-not-enabled），减少上层耦合。

- arceos_posix_api:
作用: 提供 POSIX 兼容接口层，面向 fd/pipe/select/epoll、文件与网络等 POSIX 语义的应用移植。
特点: 依赖 axfeat 的子系统开关与 axfs/axnet 等模块，将 POSIX 调用映射到 ArceOS 的内部实现。

- axfeat:
作用: 顶层“特性编排器”。统一开启/组合内核子系统与其依赖（如 alloc、paging、驱动与具体模块）。
特点: 通过单个 feature 串起整条链路（例如 net → 分配器/分页 + axdriver/virtio-net + axnet + axruntime/net），便于场景化启用。




## 5.应用运行时与库(ulib/)

- 功能定位：
ulib/ 是面向应用侧的库，而非内核功能模块。分为axstd（面向Rust），axlibc（面向C）两部分，向上提供易用，兼容API，向下通过arceos_api/arceos_posix_api驱动内核能力。

- 与内核的关系：
ulib 通过 arceos_api（Rust 风格 API）或 arceos_posix_api（POSIX 兼容 API）与内核模块交互；axfeat 负责把功能开关（如 fs/net/alloc/paging/驱动）串起来，ulib 透传这些开关来暴露或裁剪相应能力。


## 6.其它辅助目录

### axfs_ramfs

- 功能定位：

作用：提供内存文件系统实现（RamFS），被内核文件系统模块 axfs 作为后端挂载（如 /tmp，以及可复用为 procfs/sysfs）。
特点：有 Cargo.toml，是独立 Rust crate；启用相关 feature 时被链接进内核，但它本身不在 modules/ 目录中

### examles/

- 功能定位：
训练营教学示例，用于分布学习与动手实验，不属于内核。
### payload/ 
- 功能定位：

应用和示例，不属于内核，含Rust和C版本。
### tour/

- 功能定位：
课程教学材料，不属于内核。

### exercises/ 

- 功能定位：
实验教学材料，不属于内核。


### tools/ 

- 功能定位：

包含开发/调试/辅助工具，用于开发流程与环境，不属于内核
### scripts/

- 功能定位：
用于CI、构建、网络等脚本与 Make 片段，辅助工程化。不属于内核







