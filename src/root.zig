const std = @import("std");

pub const oci = @import("oci.zig");
pub const capsule = @import("capsule.zig");
pub const surge = @import("surge.zig");
pub const network = @import("network.zig");
pub const quic_fabric = @import("quic_fabric.zig");
pub const bolt_dns = @import("bolt_dns.zig");
pub const content_store = @import("content_store.zig");
pub const migration = @import("migration.zig");
pub const reproducible_builds = @import("reproducible_builds.zig");
pub const cluster = @import("cluster.zig");
pub const resource_quotas = @import("resource_quotas.zig");

pub fn runContainer(allocator: std.mem.Allocator, image_name: []const u8) !void {
    std.debug.print("Running container from image: {s}\n", .{image_name});
    
    // First, try to pull the image if not locally available
    var local_image = oci.findLocalImage(allocator, image_name) catch |err| switch (err) {
        error.ImageNotFound => blk: {
            std.debug.print("Image not found locally, pulling...\n", .{});
            try pullImage(allocator, image_name);
            break :blk try oci.findLocalImage(allocator, image_name);
        },
        else => return err,
    };
    defer local_image.deinit(allocator);
    
    // Create and start a capsule
    var container_capsule = try capsule.Capsule.init(allocator, local_image);
    defer container_capsule.deinit();
    
    try container_capsule.start();
    std.debug.print("Container started successfully\n", .{});
}

pub fn pullImage(allocator: std.mem.Allocator, image_name: []const u8) !void {
    std.debug.print("Pulling image: {s}\n", .{image_name});
    
    // Initialize content store for deduplication
    var store = try content_store.ContentAddressedStore.init(allocator, ".bolt/cas");
    defer store.deinit();
    
    var image_puller = try oci.ImagePuller.init(allocator, &store);
    defer image_puller.deinit();
    
    try image_puller.pull(image_name);
    std.debug.print("Image pulled successfully with deduplication\n", .{});
}

pub fn surgeUp(allocator: std.mem.Allocator, boltfile_path: []const u8) !void {
    std.debug.print("Starting services from: {s}\n", .{boltfile_path});
    
    var surge_orchestrator = try surge.Orchestrator.init(allocator);
    defer surge_orchestrator.deinit();
    
    try surge_orchestrator.up(boltfile_path);
    std.debug.print("Services started successfully\n", .{});
}

pub fn surgeDown(allocator: std.mem.Allocator, boltfile_path: []const u8) !void {
    std.debug.print("Stopping services from: {s}\n", .{boltfile_path});
    
    var surge_orchestrator = try surge.Orchestrator.init(allocator);
    defer surge_orchestrator.deinit();
    
    try surge_orchestrator.down(boltfile_path);
    std.debug.print("Services stopped successfully\n", .{});
}

pub fn surgeKill(allocator: std.mem.Allocator, boltfile_path: []const u8) !void {
    std.debug.print("Force killing services from: {s}\n", .{boltfile_path});
    
    var surge_orchestrator = try surge.Orchestrator.init(allocator);
    defer surge_orchestrator.deinit();
    
    try surge_orchestrator.kill(boltfile_path);
    std.debug.print("Services force killed successfully\n", .{});
}

test "basic functionality test" {
    const allocator = std.testing.allocator;
    _ = allocator;
    // Basic tests will be added as we implement features
}