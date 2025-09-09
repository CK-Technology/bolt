const std = @import("std");
const crypto = std.crypto;

pub const ContentStoreError = error{
    HashMismatch,
    ContentNotFound,
    StorageError,
    InvalidContent,
};

pub const ContentHash = struct {
    algorithm: HashAlgorithm = .sha256,
    value: [32]u8, // SHA256 is 32 bytes
    
    pub const HashAlgorithm = enum {
        sha256,
        blake3,
    };
    
    pub fn format(self: ContentHash, comptime fmt: []const u8, options: anytype, writer: anytype) !void {
        _ = fmt;
        _ = options;
        
        try writer.writeAll("sha256:");
        for (self.value) |byte| {
            try writer.print("{x:0>2}", .{byte});
        }
    }
    
    pub fn fromString(allocator: std.mem.Allocator, hash_str: []const u8) !ContentHash {
        _ = allocator;
        
        if (!std.mem.startsWith(u8, hash_str, "sha256:")) {
            return ContentStoreError.InvalidContent;
        }
        
        var hash = ContentHash{
            .algorithm = .sha256,
            .value = undefined,
        };
        
        const hex_str = hash_str["sha256:".len..];
        if (hex_str.len != 64) {
            return ContentStoreError.InvalidContent;
        }
        
        var i: usize = 0;
        while (i < 32) : (i += 1) {
            const byte_str = hex_str[i * 2 .. i * 2 + 2];
            hash.value[i] = try std.fmt.parseInt(u8, byte_str, 16);
        }
        
        return hash;
    }
};

pub const ContentObject = struct {
    hash: ContentHash,
    size: u64,
    content_type: ContentType,
    metadata: std.StringHashMap([]const u8),
    
    pub const ContentType = enum {
        layer,      // Container layer
        manifest,   // Image manifest
        config,     // Container config
        capsule,    // Capsule snapshot
        build,      // Build artifact
    };
    
    pub fn deinit(self: *ContentObject, allocator: std.mem.Allocator) void {
        var metadata_iter = self.metadata.iterator();
        while (metadata_iter.next()) |entry| {
            allocator.free(entry.key_ptr.*);
            allocator.free(entry.value_ptr.*);
        }
        self.metadata.deinit();
    }
};

pub const ContentAddressedStore = struct {
    allocator: std.mem.Allocator,
    store_path: []const u8,
    index: std.HashMap(ContentHash, ContentObject, ContentHashContext, 80),
    dedup_cache: std.StringHashMap(ContentHash), // Path -> Hash for deduplication
    
    const ContentHashContext = struct {
        pub fn hash(_: ContentHashContext, key: ContentHash) u64 {
            var hasher = std.hash.Wyhash.init(0);
            hasher.update(&key.value);
            return hasher.final();
        }
        
        pub fn eql(_: ContentHashContext, a: ContentHash, b: ContentHash) bool {
            return std.mem.eql(u8, &a.value, &b.value);
        }
    };
    
    pub fn init(allocator: std.mem.Allocator, store_path: []const u8) !ContentAddressedStore {
        // Create store directory structure
        const store_path_copy = try allocator.dupe(u8, store_path);
        
        try std.fs.cwd().makePath(store_path_copy);
        
        const objects_path = try std.fmt.allocPrint(allocator, "{s}/objects", .{store_path_copy});
        defer allocator.free(objects_path);
        try std.fs.cwd().makePath(objects_path);
        
        const temp_path = try std.fmt.allocPrint(allocator, "{s}/tmp", .{store_path_copy});
        defer allocator.free(temp_path);
        try std.fs.cwd().makePath(temp_path);
        
        return ContentAddressedStore{
            .allocator = allocator,
            .store_path = store_path_copy,
            .index = std.HashMap(ContentHash, ContentObject, ContentHashContext, 80).init(allocator),
            .dedup_cache = std.StringHashMap(ContentHash).init(allocator),
        };
    }
    
    pub fn deinit(self: *ContentAddressedStore) void {
        var index_iter = self.index.iterator();
        while (index_iter.next()) |entry| {
            var obj = entry.value_ptr;
            obj.deinit(self.allocator);
        }
        self.index.deinit();
        
        var cache_iter = self.dedup_cache.iterator();
        while (cache_iter.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
        }
        self.dedup_cache.deinit();
        
        self.allocator.free(self.store_path);
    }
    
    pub fn addContent(self: *ContentAddressedStore, content: []const u8, content_type: ContentObject.ContentType) !ContentHash {
        // Calculate hash
        var hash_value: [32]u8 = undefined;
        crypto.hash.sha2.Sha256.hash(content, &hash_value, .{});
        
        const content_hash = ContentHash{
            .algorithm = .sha256,
            .value = hash_value,
        };
        
        // Check if content already exists (deduplication)
        if (self.index.contains(content_hash)) {
            std.debug.print("[ContentStore] Content already exists: {any}\n", .{content_hash});
            return content_hash;
        }
        
        // Store content in CAS
        const object_path = try self.getObjectPath(content_hash);
        defer self.allocator.free(object_path);
        
        // Create parent directory
        const parent_path = std.fs.path.dirname(object_path) orelse return ContentStoreError.StorageError;
        try std.fs.cwd().makePath(parent_path);
        
        // Write content atomically
        const temp_path = try std.fmt.allocPrint(self.allocator, "{s}/tmp/{any}", .{ self.store_path, content_hash });
        defer self.allocator.free(temp_path);
        
        const file = try std.fs.cwd().createFile(temp_path, .{});
        defer file.close();
        
        try file.writeAll(content);
        
        // Move to final location
        try std.fs.cwd().rename(temp_path, object_path);
        
        // Add to index
        const object = ContentObject{
            .hash = content_hash,
            .size = content.len,
            .content_type = content_type,
            .metadata = std.StringHashMap([]const u8).init(self.allocator),
        };
        
        try self.index.put(content_hash, object);
        
        std.debug.print("[ContentStore] Added content: {any} ({d} bytes)\n", .{ content_hash, content.len });
        return content_hash;
    }
    
    pub fn getContent(self: *ContentAddressedStore, hash: ContentHash) ![]u8 {
        if (!self.index.contains(hash)) {
            return ContentStoreError.ContentNotFound;
        }
        
        const object_path = try self.getObjectPath(hash);
        defer self.allocator.free(object_path);
        
        const file = try std.fs.cwd().openFile(object_path, .{});
        defer file.close();
        
        const size = try file.getEndPos();
        const content = try self.allocator.alloc(u8, size);
        _ = try file.read(content);
        
        // Verify hash
        var verify_hash: [32]u8 = undefined;
        crypto.hash.sha2.Sha256.hash(content, &verify_hash, .{});
        
        if (!std.mem.eql(u8, &verify_hash, &hash.value)) {
            self.allocator.free(content);
            return ContentStoreError.HashMismatch;
        }
        
        return content;
    }
    
    pub fn addPath(self: *ContentAddressedStore, path: []const u8, content_type: ContentObject.ContentType) !ContentHash {
        // Check dedup cache
        if (self.dedup_cache.get(path)) |cached_hash| {
            std.debug.print("[ContentStore] Using cached hash for path: {s}\n", .{path});
            return cached_hash;
        }
        
        const file = try std.fs.cwd().openFile(path, .{});
        defer file.close();
        
        const size = try file.getEndPos();
        const content = try self.allocator.alloc(u8, size);
        defer self.allocator.free(content);
        
        _ = try file.read(content);
        
        const hash = try self.addContent(content, content_type);
        
        // Add to dedup cache
        const path_copy = try self.allocator.dupe(u8, path);
        try self.dedup_cache.put(path_copy, hash);
        
        return hash;
    }
    
    fn getObjectPath(self: *ContentAddressedStore, hash: ContentHash) ![]u8 {
        // Use first 2 bytes of hash for directory sharding (like Git)
        const hash_str = try std.fmt.allocPrint(self.allocator, "{any}", .{hash});
        defer self.allocator.free(hash_str);
        
        const hash_part = hash_str["sha256:".len..];
        const prefix = hash_part[0..2];
        const suffix = hash_part[2..];
        
        return std.fmt.allocPrint(
            self.allocator,
            "{s}/objects/{s}/{s}",
            .{ self.store_path, prefix, suffix }
        );
    }
    
    pub fn garbageCollect(_: *ContentAddressedStore) !void {
        std.debug.print("[ContentStore] Starting garbage collection...\n", .{});
        
        // In a real implementation, this would:
        // 1. Mark all referenced objects
        // 2. Delete unreferenced objects
        // 3. Compact the store
        
        const removed: usize = 0;
        // Placeholder for actual GC logic
        
        std.debug.print("[ContentStore] Garbage collection complete. Removed {d} objects\n", .{removed});
    }
};

// Build cache for deterministic builds
pub const BuildCache = struct {
    allocator: std.mem.Allocator,
    store: *ContentAddressedStore,
    cache_index: std.StringHashMap(BuildCacheEntry),
    
    pub const BuildCacheEntry = struct {
        input_hash: ContentHash,    // Hash of build inputs
        output_hash: ContentHash,   // Hash of build output
        timestamp: i64,
        build_time_ms: u64,
        success: bool,
        
        pub fn deinit(self: *BuildCacheEntry, allocator: std.mem.Allocator) void {
            _ = self;
            _ = allocator;
        }
    };
    
    pub fn init(allocator: std.mem.Allocator, store: *ContentAddressedStore) BuildCache {
        return BuildCache{
            .allocator = allocator,
            .store = store,
            .cache_index = std.StringHashMap(BuildCacheEntry).init(allocator),
        };
    }
    
    pub fn deinit(self: *BuildCache) void {
        var iter = self.cache_index.iterator();
        while (iter.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            entry.value_ptr.deinit(self.allocator);
        }
        self.cache_index.deinit();
    }
    
    pub fn getCachedBuild(self: *BuildCache, input_hash: ContentHash) ?BuildCacheEntry {
        const key = std.fmt.allocPrint(self.allocator, "{any}", .{input_hash}) catch return null;
        defer self.allocator.free(key);
        
        return self.cache_index.get(key);
    }
    
    pub fn addCachedBuild(self: *BuildCache, input_hash: ContentHash, output_hash: ContentHash, build_time_ms: u64) !void {
        const key = try std.fmt.allocPrint(self.allocator, "{any}", .{input_hash});
        
        const entry = BuildCacheEntry{
            .input_hash = input_hash,
            .output_hash = output_hash,
            .timestamp = std.time.timestamp(),
            .build_time_ms = build_time_ms,
            .success = true,
        };
        
        try self.cache_index.put(key, entry);
        
        std.debug.print("[BuildCache] Cached build: {any} -> {any} ({d}ms)\n", .{ input_hash, output_hash, build_time_ms });
    }
};