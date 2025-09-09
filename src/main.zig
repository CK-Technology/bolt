const std = @import("std");
const bolt = @import("bolt");
const build_options = @import("build_options");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len < 2) {
        try printUsage();
        return;
    }

    const command = args[1];

    if (std.mem.eql(u8, command, "run")) {
        if (args.len < 3) {
            std.debug.print("Usage: bolt run <image>\n", .{});
            return;
        }
        try bolt.runContainer(allocator, args[2]);
    } else if (std.mem.eql(u8, command, "pull")) {
        if (args.len < 3) {
            std.debug.print("Usage: bolt pull <image>\n", .{});
            return;
        }
        try bolt.pullImage(allocator, args[2]);
    } else if (std.mem.eql(u8, command, "surge")) {
        if (args.len < 3) {
            std.debug.print("Usage: bolt surge <up|down|kill> [boltfile]\n", .{});
            return;
        }
        const surge_command = args[2];
        const boltfile = if (args.len > 3) args[3] else "Boltfile.toml";
        
        if (std.mem.eql(u8, surge_command, "up")) {
            try bolt.surgeUp(allocator, boltfile);
        } else if (std.mem.eql(u8, surge_command, "down")) {
            try bolt.surgeDown(allocator, boltfile);
        } else if (std.mem.eql(u8, surge_command, "kill")) {
            try bolt.surgeKill(allocator, boltfile);
        } else {
            std.debug.print("Unknown surge command: {s}\n", .{surge_command});
        }
    } else if (std.mem.eql(u8, command, "version")) {
        std.debug.print("Bolt v{s}\nNext-Generation Container Runtime & Orchestration\n", .{build_options.version});
    } else {
        std.debug.print("Unknown command: {s}\n", .{command});
        try printUsage();
    }
}

fn printUsage() !void {
    std.debug.print(
        \\Bolt - Next-Generation Container Runtime & Orchestration
        \\
        \\Usage:
        \\  bolt run <image>        Run a container from an image
        \\  bolt pull <image>       Pull an image from a registry
        \\  bolt surge up [file]    Start services defined in Boltfile
        \\  bolt surge down [file]  Stop services defined in Boltfile
        \\  bolt surge kill [file]  Force kill services defined in Boltfile
        \\  bolt version            Show version information
        \\
    , .{});
}