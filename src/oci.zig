const std = @import("std");
const ContentStore = @import("content_store.zig");

pub const ImageError = error{
    ImageNotFound,
    InvalidImageFormat,
    RegistryError,
    NetworkError,
};

pub const ImageManifest = struct {
    name: []const u8,
    tag: []const u8,
    digest: []const u8,
    layers: []Layer,
    config: ImageConfig,
    
    pub fn deinit(self: *ImageManifest, allocator: std.mem.Allocator) void {
        allocator.free(self.name);
        allocator.free(self.tag);
        allocator.free(self.digest);
        for (self.layers) |*layer| {
            layer.deinit(allocator);
        }
        allocator.free(self.layers);
        self.config.deinit(allocator);
    }
};

pub const Layer = struct {
    digest: []const u8,
    size: u64,
    media_type: []const u8,
    content_hash: ?ContentStore.ContentHash = null, // CAS hash for deduplication
    
    pub fn deinit(self: *Layer, allocator: std.mem.Allocator) void {
        allocator.free(self.digest);
        allocator.free(self.media_type);
    }
};

pub const ImageConfig = struct {
    entrypoint: ?[]const u8,
    cmd: ?[]const u8,
    env: [][]const u8,
    working_dir: ?[]const u8,
    user: ?[]const u8,
    
    pub fn deinit(self: *ImageConfig, allocator: std.mem.Allocator) void {
        if (self.entrypoint) |entrypoint| allocator.free(entrypoint);
        if (self.cmd) |cmd| allocator.free(cmd);
        for (self.env) |env_var| {
            allocator.free(env_var);
        }
        allocator.free(self.env);
        if (self.working_dir) |wd| allocator.free(wd);
        if (self.user) |user| allocator.free(user);
    }
};

pub const ImagePuller = struct {
    allocator: std.mem.Allocator,
    cache_dir: []const u8,
    content_store: *ContentStore.ContentAddressedStore,
    
    pub fn init(allocator: std.mem.Allocator, content_store: *ContentStore.ContentAddressedStore) !ImagePuller {
        // Create cache directory if it doesn't exist
        const home = std.process.getEnvVarOwned(allocator, "HOME") catch "/tmp";
        defer if (!std.mem.eql(u8, home, "/tmp")) allocator.free(home);
        const cache_dir = try std.fmt.allocPrint(allocator, "{s}/.bolt/cache", .{home});
        std.fs.cwd().makePath(cache_dir) catch {};
        
        return ImagePuller{
            .allocator = allocator,
            .cache_dir = cache_dir,
            .content_store = content_store,
        };
    }
    
    pub fn deinit(self: *ImagePuller) void {
        self.allocator.free(self.cache_dir);
    }
    
    pub fn pull(self: *ImagePuller, image_name: []const u8) !void {
        std.debug.print("[OCI] Pulling image: {s}\n", .{image_name});
        
        // Parse image name (registry/namespace/name:tag)
        const parsed = try parseImageName(self.allocator, image_name);
        defer parsed.deinit(self.allocator);
        
        // Simulate downloading layers and store them in CAS
        const mock_layer_data = "Mock layer content for deduplication testing";
        const layer_hash = try self.content_store.addContent(mock_layer_data, .layer);
        
        std.debug.print("[OCI] Layer stored in CAS: {any}\n", .{layer_hash});
        
        // Store config in CAS
        const mock_config = 
            \\{
            \\  "entrypoint": "/docker-entrypoint.sh",
            \\  "cmd": "nginx -g 'daemon off;'",
            \\  "env": ["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"],
            \\  "working_dir": "/",
            \\  "user": "root"
            \\}
        ;
        const config_hash = try self.content_store.addContent(mock_config, .config);
        std.debug.print("[OCI] Config stored in CAS: {any}\n", .{config_hash});
        
        // Create manifest with CAS references
        const manifest_with_cas = try std.fmt.allocPrint(
            self.allocator,
            \\{{
            \\  "name": "{s}",
            \\  "tag": "{s}", 
            \\  "digest": "sha256:abc123def456",
            \\  "layers": [
            \\    {{
            \\      "digest": "sha256:layer1",
            \\      "size": 1024,
            \\      "media_type": "application/vnd.oci.image.layer.v1.tar+gzip",
            \\      "cas_hash": "{any}"
            \\    }}
            \\  ],
            \\  "config_cas_hash": "{any}"
            \\}}
            ,
            .{ parsed.name, parsed.tag, layer_hash, config_hash }
        );
        defer self.allocator.free(manifest_with_cas);
        
        // Store manifest in CAS as well
        const manifest_hash = try self.content_store.addContent(manifest_with_cas, .manifest);
        std.debug.print("[OCI] Manifest stored in CAS: {any}\n", .{manifest_hash});
        
        // Also write to traditional cache for compatibility
        const manifest_path = try std.fmt.allocPrint(
            self.allocator, 
            "{s}/{s}_{s}.json", 
            .{self.cache_dir, parsed.name, parsed.tag}
        );
        defer self.allocator.free(manifest_path);
        
        const file = try std.fs.cwd().createFile(manifest_path, .{});
        defer file.close();
        try file.writeAll(manifest_with_cas);
        
        std.debug.print("[OCI] Image {s} cached with CAS deduplication\n", .{image_name});
    }
};

const ParsedImage = struct {
    registry: []const u8,
    name: []const u8,
    tag: []const u8,
    
    pub fn deinit(self: *const ParsedImage, allocator: std.mem.Allocator) void {
        allocator.free(self.registry);
        allocator.free(self.name);
        allocator.free(self.tag);
    }
};

fn parseImageName(allocator: std.mem.Allocator, image_name: []const u8) !ParsedImage {
    // Simple parser for now: assumes format like "nginx:latest" or "registry.io/nginx:latest"
    var parts = std.mem.splitSequence(u8, image_name, ":");
    const name_part = parts.next() orelse return ImageError.InvalidImageFormat;
    const tag = parts.next() orelse "latest";
    
    // Check if name contains registry
    var registry: []const u8 = "docker.io";
    var name: []const u8 = name_part;
    
    if (std.mem.indexOf(u8, name_part, "/")) |_| {
        var name_parts = std.mem.splitSequence(u8, name_part, "/");
        const first_part = name_parts.next().?;
        if (std.mem.indexOf(u8, first_part, ".")) |_| {
            registry = first_part;
            name = name_parts.rest();
        }
    }
    
    return ParsedImage{
        .registry = try allocator.dupe(u8, registry),
        .name = try allocator.dupe(u8, name),
        .tag = try allocator.dupe(u8, tag),
    };
}

pub fn findLocalImage(allocator: std.mem.Allocator, image_name: []const u8) !ImageManifest {
    const home = std.process.getEnvVarOwned(allocator, "HOME") catch "/tmp";
    defer if (!std.mem.eql(u8, home, "/tmp")) allocator.free(home);
    const cache_dir = try std.fmt.allocPrint(allocator, "{s}/.bolt/cache", .{home});
    defer allocator.free(cache_dir);
    
    const parsed = try parseImageName(allocator, image_name);
    defer parsed.deinit(allocator);
    
    const manifest_path = try std.fmt.allocPrint(
        allocator, 
        "{s}/{s}_{s}.json", 
        .{cache_dir, parsed.name, parsed.tag}
    );
    defer allocator.free(manifest_path);
    
    // Try to read the manifest file
    const file = std.fs.cwd().openFile(manifest_path, .{}) catch {
        return ImageError.ImageNotFound;
    };
    defer file.close();
    
    // Create properly initialized layer
    var layers = try allocator.alloc(Layer, 1);
    layers[0] = Layer{
        .digest = try allocator.dupe(u8, "sha256:layer1"),
        .size = 1024,
        .media_type = try allocator.dupe(u8, "application/vnd.oci.image.layer.v1.tar+gzip"),
    };
    
    // Create environment variables array
    var env_vars = try allocator.alloc([]const u8, 1);
    env_vars[0] = try allocator.dupe(u8, "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin");
    
    // For now, return a mock manifest
    return ImageManifest{
        .name = try allocator.dupe(u8, parsed.name),
        .tag = try allocator.dupe(u8, parsed.tag),
        .digest = try allocator.dupe(u8, "sha256:abc123def456"),
        .layers = layers,
        .config = ImageConfig{
            .entrypoint = try allocator.dupe(u8, "/docker-entrypoint.sh"),
            .cmd = try allocator.dupe(u8, "nginx -g 'daemon off;'"),
            .env = env_vars,
            .working_dir = try allocator.dupe(u8, "/"),
            .user = try allocator.dupe(u8, "root"),
        },
    };
}