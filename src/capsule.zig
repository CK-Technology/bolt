const std = @import("std");
const oci = @import("oci.zig");
const linux = std.os.linux;

pub const CapsuleError = error{
    NamespaceCreationFailed,
    CgroupCreationFailed,
    MountFailed,
    ExecFailed,
    InvalidConfiguration,
};

pub const CapsuleConfig = struct {
    name: []const u8,
    hostname: []const u8,
    memory_limit: ?u64 = null,
    cpu_limit: ?f32 = null,
    rootfs_path: []const u8,
    working_dir: []const u8 = "/",
    user: []const u8 = "root",
    env: [][]const u8,
    mounts: []Mount = &.{},
    
    pub const Mount = struct {
        source: []const u8,
        target: []const u8,
        fs_type: []const u8 = "bind",
        flags: u32 = linux.MS.BIND,
    };
};

pub const Capsule = struct {
    allocator: std.mem.Allocator,
    config: CapsuleConfig,
    pid: ?std.process.Child.Id = null,
    cgroup_path: ?[]const u8 = null,
    
    pub fn init(allocator: std.mem.Allocator, image: oci.ImageManifest) !Capsule {
        // Create capsule working directory
        const capsule_id = try generateCapsuleId(allocator);
        defer allocator.free(capsule_id);
        
        const capsule_dir = try std.fmt.allocPrint(allocator, "/tmp/bolt/capsules/{s}", .{capsule_id});
        defer allocator.free(capsule_dir);
        const rootfs_path = try std.fmt.allocPrint(allocator, "{s}/rootfs", .{capsule_dir});
        
        // Create directories
        try std.fs.cwd().makePath(capsule_dir);
        try std.fs.cwd().makePath(rootfs_path);
        
        // Extract image layers (simplified for now)
        try extractImageLayers(allocator, image, rootfs_path);
        
        const config = CapsuleConfig{
            .name = try allocator.dupe(u8, capsule_id),
            .hostname = try allocator.dupe(u8, capsule_id),
            .rootfs_path = rootfs_path,
            .working_dir = try allocator.dupe(u8, image.config.working_dir orelse "/"),
            .user = try allocator.dupe(u8, image.config.user orelse "root"),
            .env = blk: {
                const env_copy = try allocator.alloc([]const u8, image.config.env.len);
                for (image.config.env, 0..) |env_var, i| {
                    env_copy[i] = try allocator.dupe(u8, env_var);
                }
                break :blk env_copy;
            },
        };
        
        return Capsule{
            .allocator = allocator,
            .config = config,
        };
    }
    
    pub fn deinit(self: *Capsule) void {
        self.allocator.free(self.config.name);
        self.allocator.free(self.config.hostname);
        self.allocator.free(self.config.rootfs_path);
        self.allocator.free(self.config.working_dir);
        self.allocator.free(self.config.user);
        
        for (self.config.env) |env_var| {
            self.allocator.free(env_var);
        }
        self.allocator.free(self.config.env);
        
        if (self.cgroup_path) |path| {
            self.allocator.free(path);
        }
    }
    
    pub fn start(self: *Capsule) !void {
        std.debug.print("[Capsule] Starting capsule: {s}\n", .{self.config.name});
        
        // Create cgroup for resource limits
        try self.createCgroup();
        
        // Create namespaces and start the container process
        const pid = std.posix.fork() catch |err| {
            std.debug.print("[Capsule] Warning: Fork failed: {any}, running in current process\n", .{err});
            try self.setupContainer();
            return;
        };
        
        if (pid == 0) {
            // Child process - this will become the container
            try self.setupContainer();
            try self.execContainer();
        } else {
            // Parent process
            self.pid = pid;
            std.debug.print("[Capsule] Container started with PID: {d}\n", .{pid});
            
            // Wait for container to finish (for now)
            const result = std.posix.waitpid(pid, 0);
            std.debug.print("[Capsule] Container exited with status: {d}\n", .{result.status});
        }
    }
    
    fn createCgroup(self: *Capsule) !void {
        // Create cgroup v2 for this capsule
        const cgroup_path = try std.fmt.allocPrint(
            self.allocator, 
            "/sys/fs/cgroup/bolt/{s}", 
            .{self.config.name}
        );
        
        // Create the cgroup directory
        std.fs.cwd().makePath(cgroup_path) catch |err| switch (err) {
            error.AccessDenied => {
                std.debug.print("[Capsule] Warning: Cannot create cgroup (need root privileges)\n", .{});
                return;
            },
            else => return err,
        };
        
        self.cgroup_path = cgroup_path;
        
        // Set memory limit if specified
        if (self.config.memory_limit) |memory_limit| {
            const memory_max_path = try std.fmt.allocPrint(self.allocator, "{s}/memory.max", .{cgroup_path});
            defer self.allocator.free(memory_max_path);
            
            const memory_max_file = std.fs.cwd().createFile(memory_max_path, .{}) catch |err| switch (err) {
                error.AccessDenied => {
                    std.debug.print("[Capsule] Warning: Cannot set memory limit (need root privileges)\n", .{});
                    return;
                },
                else => return err,
            };
            defer memory_max_file.close();
            
            const writer_content = try std.fmt.allocPrint(self.allocator, "{d}\n", .{memory_limit});
            defer self.allocator.free(writer_content);
            try memory_max_file.writeAll(writer_content);
        }
        
        std.debug.print("[Capsule] Cgroup created: {s}\n", .{cgroup_path});
    }
    
    fn setupContainer(self: *Capsule) !void {
        // Create new namespaces
        const unshare_flags = linux.CLONE.NEWPID | linux.CLONE.NEWNET | linux.CLONE.NEWNS | linux.CLONE.NEWUTS;
        
        const result = linux.unshare(unshare_flags);
        if (result != 0) {
            std.debug.print("[Capsule] Warning: Failed to create namespaces (need root privileges)\n", .{});
        }
        
        // Set hostname using direct syscall
        const hostname_result = linux.syscall2(.sethostname, @intFromPtr(self.config.hostname.ptr), self.config.hostname.len);
        if (hostname_result != 0) {
            std.debug.print("[Capsule] Warning: Failed to set hostname (errno: {any})\n", .{hostname_result});
        } else {
            std.debug.print("[Capsule] Set hostname to: {s}\n", .{self.config.hostname});
        }
        
        // Change root to container rootfs
        std.posix.chdir(self.config.rootfs_path) catch |err| {
            std.debug.print("[Capsule] Warning: Failed to chdir to rootfs: {any}\n", .{err});
            return;
        };
        
        // Mount basic filesystems (if we have permissions)
        self.mountBasicFilesystems() catch |err| {
            std.debug.print("[Capsule] Warning: Failed to mount basic filesystems: {any}\n", .{err});
        };
        
        // Chroot to the new root
        const chroot_result = linux.chroot(".");
        if (chroot_result != 0) {
            std.debug.print("[Capsule] Warning: Failed to chroot (need root privileges)\n", .{});
        }
        
        // Change to working directory
        std.posix.chdir(self.config.working_dir) catch |err| {
            std.debug.print("[Capsule] Warning: Failed to chdir to working dir: {any}\n", .{err});
        };
    }
    
    fn mountBasicFilesystems(self: *Capsule) !void {
        _ = self;
        // Mount /proc
        const proc_result = linux.mount("proc", "/proc", "proc", 0, 0);
        if (proc_result != 0) {
            return;
        }
        
        // Mount /sys  
        const sys_result = linux.mount("sysfs", "/sys", "sysfs", 0, 0);
        if (sys_result != 0) {
            return;
        }
        
        // Mount /dev/pts for terminals
        const devpts_result = linux.mount("devpts", "/dev/pts", "devpts", 0, @intFromPtr("newinstance,ptmxmode=0666".ptr));
        _ = devpts_result; // Ignore errors for now
    }
    
    fn execContainer(self: *Capsule) !void {
        _ = self;
        // For now, just run a simple shell
        const args = [_:null]?[*:0]const u8{ "/bin/sh", "-c", "echo 'Hello from Bolt Capsule!'; /bin/sh", null };
        const env = [_:null]?[*:0]const u8{ "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin", "HOME=/root", null };
        
        const exec_result = linux.execve("/bin/sh", &args, &env);
        if (exec_result != 0) {
            std.debug.print("[Capsule] Failed to exec container process\n", .{});
            std.process.exit(1);
        }
    }
    
};

fn generateCapsuleId(allocator: std.mem.Allocator) ![]const u8 {
    // Generate a simple random ID for the capsule
    var prng = std.Random.DefaultPrng.init(@intCast(std.time.timestamp()));
    const random = prng.random();
    
    const id = random.int(u32);
    return try std.fmt.allocPrint(allocator, "capsule-{x}", .{id});
}

fn extractImageLayers(allocator: std.mem.Allocator, image: oci.ImageManifest, rootfs_path: []const u8) !void {
    _ = image;
    
    // For now, create a minimal rootfs structure
    const dirs = [_][]const u8{ "bin", "etc", "lib", "proc", "sys", "tmp", "usr", "var", "dev" };
    
    for (dirs) |dir| {
        const full_path = try std.fmt.allocPrint(allocator, "{s}/{s}", .{ rootfs_path, dir });
        defer allocator.free(full_path);
        
        std.fs.cwd().makePath(full_path) catch {};
    }
    
    // Create a simple shell script for testing
    const shell_path = try std.fmt.allocPrint(allocator, "{s}/bin/sh", .{rootfs_path});
    defer allocator.free(shell_path);
    
    const shell_content = "#!/bin/sh\necho 'Bolt Capsule Shell'\nexec /bin/bash \"$@\"\n";
    
    const shell_file = std.fs.cwd().createFile(shell_path, .{}) catch |err| switch (err) {
        error.AccessDenied => return,
        else => return err,
    };
    defer shell_file.close();
    
    try shell_file.writeAll(shell_content);
    
    // Make it executable
    const shell_file_handle = try std.fs.cwd().openFile(shell_path, .{});
    defer shell_file_handle.close();
    try shell_file_handle.chmod(0o755);
    
    std.debug.print("[Capsule] Basic rootfs created at: {s}\n", .{rootfs_path});
}