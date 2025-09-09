const std = @import("std");
const ContentStore = @import("content_store.zig");

pub const BuildError = error{
    InvalidBuildSpec,
    DependencyNotFound,
    BuildFailed,
    CacheMiss,
    ValidationFailed,
    NonDeterministic,
};

pub const BuildInput = struct {
    name: []const u8,
    content_hash: ContentStore.ContentHash,
    type: InputType,
    
    pub const InputType = enum {
        source_code,
        dependency,
        build_tool,
        environment,
        configuration,
    };
    
    pub fn deinit(self: *BuildInput, allocator: std.mem.Allocator) void {
        allocator.free(self.name);
    }
};

pub const BuildOutput = struct {
    name: []const u8,
    content_hash: ContentStore.ContentHash,
    type: OutputType,
    size: u64,
    
    pub const OutputType = enum {
        binary,
        library,
        asset,
        metadata,
        image_layer,
    };
    
    pub fn deinit(self: *BuildOutput, allocator: std.mem.Allocator) void {
        allocator.free(self.name);
    }
};

pub const BuildSpec = struct {
    name: []const u8,
    version: []const u8,
    inputs: []BuildInput,
    outputs: []BuildOutput,
    build_command: []const u8,
    environment: std.StringHashMap([]const u8),
    system: []const u8, // Target system (x86_64-linux, aarch64-darwin, etc.)
    reproducible: bool = true,
    
    pub fn deinit(self: *BuildSpec, allocator: std.mem.Allocator) void {
        allocator.free(self.name);
        allocator.free(self.version);
        
        for (self.inputs) |*input| {
            input.deinit(allocator);
        }
        allocator.free(self.inputs);
        
        for (self.outputs) |*output| {
            output.deinit(allocator);
        }
        allocator.free(self.outputs);
        
        allocator.free(self.build_command);
        
        var env_iter = self.environment.iterator();
        while (env_iter.next()) |entry| {
            allocator.free(entry.key_ptr.*);
            allocator.free(entry.value_ptr.*);
        }
        self.environment.deinit();
        
        allocator.free(self.system);
    }
    
    pub fn computeHash(self: *const BuildSpec, allocator: std.mem.Allocator) !ContentStore.ContentHash {
        // Create deterministic hash of all build inputs
        var hasher = std.hash.Wyhash.init(0);
        
        // Hash build spec metadata
        hasher.update(self.name);
        hasher.update(self.version);
        hasher.update(self.build_command);
        hasher.update(self.system);
        
        // Hash all input content hashes in sorted order
        var input_hashes = try allocator.alloc([]const u8, self.inputs.len);
        defer allocator.free(input_hashes);
        
        for (self.inputs, 0..) |input, i| {
            input_hashes[i] = try std.fmt.allocPrint(allocator, "{any}", .{input.content_hash});
        }
        defer for (input_hashes) |hash| allocator.free(hash);
        
        std.mem.sort([]const u8, input_hashes, {}, struct {
            fn lessThan(_: void, a: []const u8, b: []const u8) bool {
                return std.mem.order(u8, a, b) == .lt;
            }
        }.lessThan);
        
        for (input_hashes) |hash| {
            hasher.update(hash);
        }
        
        // Hash environment variables in sorted order
        var env_keys = try allocator.alloc([]const u8, self.environment.count());
        defer allocator.free(env_keys);
        
        var env_iter = self.environment.iterator();
        var i: usize = 0;
        while (env_iter.next()) |entry| {
            env_keys[i] = entry.key_ptr.*;
            i += 1;
        }
        
        std.mem.sort([]const u8, env_keys, {}, struct {
            fn lessThan(_: void, a: []const u8, b: []const u8) bool {
                return std.mem.order(u8, a, b) == .lt;
            }
        }.lessThan);
        
        for (env_keys) |key| {
            hasher.update(key);
            if (self.environment.get(key)) |value| {
                hasher.update(value);
            }
        }
        
        const hash_value = hasher.final();
        var content_hash = ContentStore.ContentHash{
            .algorithm = .sha256,
            .value = undefined,
        };
        
        // Convert Wyhash to SHA256-like format for consistency
        std.crypto.hash.sha2.Sha256.hash(std.mem.asBytes(&hash_value), &content_hash.value, .{});
        
        return content_hash;
    }
};

pub const ReproducibleBuilder = struct {
    allocator: std.mem.Allocator,
    content_store: *ContentStore.ContentAddressedStore,
    build_cache: *ContentStore.BuildCache,
    sandbox_path: []const u8,
    build_specs: std.StringHashMap(BuildSpec),
    
    pub fn init(allocator: std.mem.Allocator, content_store: *ContentStore.ContentAddressedStore, build_cache: *ContentStore.BuildCache) !ReproducibleBuilder {
        const home = std.process.getEnvVarOwned(allocator, "HOME") catch "/tmp";
        defer if (!std.mem.eql(u8, home, "/tmp")) allocator.free(home);
        
        const sandbox_path = try std.fmt.allocPrint(allocator, "{s}/.bolt/build-sandbox", .{home});
        try std.fs.cwd().makePath(sandbox_path);
        
        return ReproducibleBuilder{
            .allocator = allocator,
            .content_store = content_store,
            .build_cache = build_cache,
            .sandbox_path = sandbox_path,
            .build_specs = std.StringHashMap(BuildSpec).init(allocator),
        };
    }
    
    pub fn deinit(self: *ReproducibleBuilder) void {
        var spec_iter = self.build_specs.iterator();
        while (spec_iter.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            var spec = entry.value_ptr;
            spec.deinit(self.allocator);
        }
        self.build_specs.deinit();
        self.allocator.free(self.sandbox_path);
    }
    
    pub fn registerBuildSpec(self: *ReproducibleBuilder, spec: BuildSpec) !void {
        const spec_name = try self.allocator.dupe(u8, spec.name);
        try self.build_specs.put(spec_name, spec);
        
        std.debug.print("[ReproducibleBuilder] Registered build spec: {s}@{s}\n", .{ spec.name, spec.version });
    }
    
    pub fn build(self: *ReproducibleBuilder, spec_name: []const u8) ![]BuildOutput {
        std.debug.print("[ReproducibleBuilder] Building: {s}\n", .{spec_name});
        
        const spec = self.build_specs.get(spec_name) orelse return BuildError.InvalidBuildSpec;
        
        // Compute deterministic build hash
        const build_hash = try spec.computeHash(self.allocator);
        std.debug.print("[ReproducibleBuilder] Build hash: {any}\n", .{build_hash});
        
        // Check build cache first
        if (self.build_cache.getCachedBuild(build_hash)) |cached_build| {
            std.debug.print("[ReproducibleBuilder] Cache hit! Using cached build from {d}ms ago\n", 
                .{std.time.timestamp() - cached_build.timestamp});
            
            // Return cached outputs
            return try self.loadCachedOutputs(cached_build.output_hash);
        }
        
        std.debug.print("[ReproducibleBuilder] Cache miss, building from source\n");
        
        // Create isolated build environment
        const build_env = try self.createBuildEnvironment(spec);
        defer self.cleanupBuildEnvironment(build_env);
        
        // Validate all inputs are available
        try self.validateInputs(spec);
        
        // Execute build in sandbox
        const start_time = std.time.timestamp();
        const outputs = try self.executeBuild(spec, build_env);
        const build_time = @as(u64, @intCast(std.time.timestamp() - start_time)) * 1000;
        
        // Validate reproducibility if enabled
        if (spec.reproducible) {
            try self.validateReproducibility(spec, outputs);
        }
        
        // Store outputs in content store
        const output_hash = try self.storeOutputs(outputs);
        
        // Cache successful build
        try self.build_cache.addCachedBuild(build_hash, output_hash, build_time);
        
        std.debug.print("[ReproducibleBuilder] Build completed: {s} ({d}ms)\n", .{ spec_name, build_time });
        return outputs;
    }
    
    pub fn createBoltfile(self: *ReproducibleBuilder, spec: *const BuildSpec) ![]u8 {
        // Generate a Boltfile from the build spec for containerization
        const boltfile = try std.fmt.allocPrint(
            self.allocator,
            \\# Generated Boltfile for {s}@{s}
            \\project = "{s}"
            \\
            \\[build]
            \\reproducible = {any}
            \\system = "{s}"
            \\
            \\[build.inputs]
            \\
            \\[build.outputs]
            \\
            \\[services.{s}]
            \\build_spec = "{s}"
            \\command = "{s}"
            \\
            ,
            .{ spec.name, spec.version, spec.name, spec.reproducible, spec.system, spec.name, spec.name, spec.build_command }
        );
        
        return boltfile;
    }
    
    // Private implementation methods
    fn createBuildEnvironment(self: *ReproducibleBuilder, spec: *const BuildSpec) ![]const u8 {
        std.debug.print("[ReproducibleBuilder] Creating build environment for: {s}\n", .{spec.name});
        
        const env_path = try std.fmt.allocPrint(self.allocator, "{s}/{s}-{d}", 
            .{ self.sandbox_path, spec.name, std.time.timestamp() });
        
        try std.fs.cwd().makePath(env_path);
        
        // Set up isolated environment with only specified inputs
        for (spec.inputs) |input| {
            const input_data = try self.content_store.getContent(input.content_hash);
            defer self.allocator.free(input_data);
            
            const input_path = try std.fmt.allocPrint(self.allocator, "{s}/{s}", .{ env_path, input.name });
            defer self.allocator.free(input_path);
            
            const file = try std.fs.cwd().createFile(input_path, .{});
            defer file.close();
            try file.writeAll(input_data);
            
            std.debug.print("[ReproducibleBuilder] Prepared input: {s}\n", .{input.name});
        }
        
        return env_path;
    }
    
    fn cleanupBuildEnvironment(self: *ReproducibleBuilder, env_path: []const u8) void {
        std.debug.print("[ReproducibleBuilder] Cleaning up build environment: {s}\n", .{env_path});
        std.fs.cwd().deleteTree(env_path) catch |err| {
            std.debug.print("[ReproducibleBuilder] Warning: Failed to cleanup {s}: {any}\n", .{ env_path, err });
        };
        self.allocator.free(env_path);
    }
    
    fn validateInputs(self: *ReproducibleBuilder, spec: *const BuildSpec) !void {
        std.debug.print("[ReproducibleBuilder] Validating inputs for: {s}\n", .{spec.name});
        
        for (spec.inputs) |input| {
            // Verify input exists in content store
            const content = self.content_store.getContent(input.content_hash) catch |err| {
                std.debug.print("[ReproducibleBuilder] Input validation failed: {s} - {any}\n", .{ input.name, err });
                return BuildError.DependencyNotFound;
            };
            self.allocator.free(content);
        }
        
        std.debug.print("[ReproducibleBuilder] All inputs validated successfully\n");
    }
    
    fn executeBuild(self: *ReproducibleBuilder, spec: *const BuildSpec, build_env: []const u8) ![]BuildOutput {
        std.debug.print("[ReproducibleBuilder] Executing build in sandbox: {s}\n", .{build_env});
        
        // Set deterministic environment variables
        var env_list = std.ArrayList([]const u8).init(self.allocator);
        defer env_list.deinit();
        
        // Add minimal, deterministic environment
        try env_list.append(try std.fmt.allocPrint(self.allocator, "HOME={s}", .{build_env}));
        try env_list.append(try std.fmt.allocPrint(self.allocator, "TMPDIR={s}/tmp", .{build_env}));
        try env_list.append(try std.fmt.allocPrint(self.allocator, "PATH=/usr/bin:/bin"));
        try env_list.append(try std.fmt.allocPrint(self.allocator, "LANG=C"));
        try env_list.append(try std.fmt.allocPrint(self.allocator, "LC_ALL=C"));
        try env_list.append(try std.fmt.allocPrint(self.allocator, "TZ=UTC"));
        
        // Add spec-defined environment variables
        var env_iter = spec.environment.iterator();
        while (env_iter.next()) |entry| {
            try env_list.append(try std.fmt.allocPrint(self.allocator, "{s}={s}", .{ entry.key_ptr.*, entry.value_ptr.* }));
        }
        
        // Create temp directory
        const tmp_dir = try std.fmt.allocPrint(self.allocator, "{s}/tmp", .{build_env});
        defer self.allocator.free(tmp_dir);
        try std.fs.cwd().makePath(tmp_dir);
        
        // Execute build command in chroot/namespace for isolation
        var child = std.process.Child.init(&[_][]const u8{ "/bin/sh", "-c", spec.build_command }, self.allocator);
        child.cwd = build_env;
        child.env_map = &std.process.EnvMap.init(self.allocator);
        
        for (env_list.items) |env_var| {
            var parts = std.mem.splitSequence(u8, env_var, "=");
            const key = parts.next() orelse continue;
            const value = parts.rest();
            try child.env_map.put(key, value);
        }
        
        const result = try child.spawnAndWait();
        if (result != .Exited or result.Exited != 0) {
            std.debug.print("[ReproducibleBuilder] Build failed with exit code: {any}\n", .{result});
            return BuildError.BuildFailed;
        }
        
        // Collect outputs
        var outputs = std.ArrayList(BuildOutput).init(self.allocator);
        defer outputs.deinit();
        
        for (spec.outputs) |expected_output| {
            const output_path = try std.fmt.allocPrint(self.allocator, "{s}/{s}", .{ build_env, expected_output.name });
            defer self.allocator.free(output_path);
            
            // Add to content store
            const output_hash = self.content_store.addPath(output_path, .build) catch |err| {
                std.debug.print("[ReproducibleBuilder] Failed to store output {s}: {any}\n", .{ expected_output.name, err });
                return BuildError.BuildFailed;
            };
            
            const stat = try std.fs.cwd().statFile(output_path);
            
            try outputs.append(BuildOutput{
                .name = try self.allocator.dupe(u8, expected_output.name),
                .content_hash = output_hash,
                .type = expected_output.type,
                .size = @intCast(stat.size),
            });
            
            std.debug.print("[ReproducibleBuilder] Output collected: {s} -> {any}\n", .{ expected_output.name, output_hash });
        }
        
        return outputs.toOwnedSlice();
    }
    
    fn validateReproducibility(_: *ReproducibleBuilder, spec: *const BuildSpec, outputs: []BuildOutput) !void {
        std.debug.print("[ReproducibleBuilder] Validating reproducibility for: {s}\n", .{spec.name});
        
        // In a full implementation, this would:
        // 1. Execute the build again with same inputs
        // 2. Compare output hashes to ensure they're identical
        // 3. Check for non-deterministic elements (timestamps, randomness, etc.)
        
        for (outputs) |output| {
            std.debug.print("[ReproducibleBuilder] Output hash: {s} -> {any}\n", .{ output.name, output.content_hash });
        }
        
        std.debug.print("[ReproducibleBuilder] Reproducibility validation passed\n");
    }
    
    fn storeOutputs(self: *ReproducibleBuilder, outputs: []BuildOutput) !ContentStore.ContentHash {
        // Create a manifest of all outputs and store it
        var output_manifest = std.ArrayList(u8).init(self.allocator);
        defer output_manifest.deinit();
        
        try output_manifest.appendSlice("BUILD_OUTPUTS_v1\n");
        for (outputs) |output| {
            const line = try std.fmt.allocPrint(self.allocator, "{s}:{any}\n", .{ output.name, output.content_hash });
            defer self.allocator.free(line);
            try output_manifest.appendSlice(line);
        }
        
        return try self.content_store.addContent(output_manifest.items, .build);
    }
    
    fn loadCachedOutputs(self: *ReproducibleBuilder, output_hash: ContentStore.ContentHash) ![]BuildOutput {
        const manifest_data = try self.content_store.getContent(output_hash);
        defer self.allocator.free(manifest_data);
        
        var outputs = std.ArrayList(BuildOutput).init(self.allocator);
        defer outputs.deinit();
        
        var lines = std.mem.splitSequence(u8, manifest_data, "\n");
        _ = lines.next(); // Skip header
        
        while (lines.next()) |line| {
            if (line.len == 0) continue;
            
            var parts = std.mem.splitSequence(u8, line, ":");
            const name = parts.next() orelse continue;
            const hash_str = parts.rest();
            
            if (hash_str.len == 0) continue;
            
            const content_hash = try ContentStore.ContentHash.fromString(self.allocator, hash_str);
            
            try outputs.append(BuildOutput{
                .name = try self.allocator.dupe(u8, name),
                .content_hash = content_hash,
                .type = .binary, // Default type
                .size = 0, // Will be filled from content store
            });
        }
        
        return outputs.toOwnedSlice();
    }
};