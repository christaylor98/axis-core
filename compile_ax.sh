#!/usr/bin/env bash
set -euo pipefail

# Axis Compiler - Compile .ax source files to executable binaries
# Usage: ./compile_ax.sh <source.ax> [output_binary]

# This file simulates a simplified build and compile process for the Axis system, providing a convenient way to explore compiler
# and bridge interactions.

# Print usage and exit
usage() {
    echo "Usage: $0 [options] <source.ax> [source2.ax ...] [output_binary]"
    echo ""
    echo "Compiles Axis source files to an executable binary."
    echo ""
    echo "Arguments:"
    echo "  source.ax       Input Axis source file(s) (at least one required)"
    echo "  output_binary   Output executable name (optional, defaults to first source name without extension)"
    echo "                  Use -o/--output to specify output when using multiple source files"
    echo ""
    echo "Options:"
    echo "  -o, --output <name>    Output executable name"
    echo "  -r, --registry <file>  Registry file to use (can be specified multiple times)"
    echo "                         Defaults to registries/axis.axreg if not specified"
    echo "  -h, --help            Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 examples/hello.ax"
    echo "  $0 examples/hello.ax my_program"
    echo "  $0 main.ax utils.ax helpers.ax -o myapp"
    echo "  $0 -r custom.axreg examples/hello.ax"
    echo "  $0 -r reg1.axreg -r reg2.axreg src/*.ax -o program"
    exit 1
}

# Parse command-line options
REGISTRY_FILES=()
SOURCE_FILES=()
OUTPUT_BINARY=""
OUTPUT_SPECIFIED=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)
            usage
            ;;
        -o|--output)
            if [[ -z "${2:-}" ]] || [[ "$2" == -* ]]; then
                echo "Error: --output requires a name argument"
                echo ""
                usage
            fi
            OUTPUT_BINARY="$2"
            OUTPUT_SPECIFIED=true
            shift 2
            ;;
        -r|--registry)
            if [[ -z "${2:-}" ]] || [[ "$2" == -* ]]; then
                echo "Error: --registry requires a file path argument"
                echo ""
                usage
            fi
            REGISTRY_FILES+=("$2")
            shift 2
            ;;
        -*)
            echo "Error: Unknown option: $1"
            echo ""
            usage
            ;;
        *)
            # Positional arguments - collect as source files
            SOURCE_FILES+=("$1")
            shift
            ;;
    esac
done

# Check if at least one source file is provided
if [[ ${#SOURCE_FILES[@]} -eq 0 ]]; then
    echo "Error: No source files provided"
    echo ""
    usage
fi

# Validate source files and separate output binary if not using -o
VALIDATED_SOURCES=()
for file in "${SOURCE_FILES[@]}"; do
    if [[ "$file" =~ \.ax$ ]]; then
        # This is a source file
        if [[ ! -f "$file" ]]; then
            echo "Error: Source file '$file' not found"
            exit 1
        fi
        VALIDATED_SOURCES+=("$file")
    elif [[ ${#VALIDATED_SOURCES[@]} -gt 0 ]] && [[ -z "$OUTPUT_BINARY" ]] && ! $OUTPUT_SPECIFIED; then
        # Last non-.ax argument might be output binary name
        OUTPUT_BINARY="$file"
    else
        echo "Error: Invalid file '$file' - source files must have .ax extension"
        exit 1
    fi
done

if [[ ${#VALIDATED_SOURCES[@]} -eq 0 ]]; then
    echo "Error: No valid .ax source files provided"
    exit 1
fi

# Determine output binary name
if [[ -z "$OUTPUT_BINARY" ]]; then
    # Use first source file's base name without extension
    BASENAME=$(basename "${VALIDATED_SOURCES[0]}" .ax)
    OUTPUT_BINARY="$BASENAME"
fi

# Convert output to absolute path if it's not already
if [[ "$OUTPUT_BINARY" != /* ]]; then
    OUTPUT_BINARY="$(pwd)/$OUTPUT_BINARY"
fi

# Get absolute paths
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Convert all source files to absolute paths
SOURCE_ABS_LIST=()
for src in "${VALIDATED_SOURCES[@]}"; do
    SRC_ABS="$(cd "$(dirname "$src")" && pwd)/$(basename "$src")"
    SOURCE_ABS_LIST+=("$SRC_ABS")
done

# Handle registry files
if [[ ${#REGISTRY_FILES[@]} -eq 0 ]]; then
    # Use default registry
    REGISTRY_FILES=("$SCRIPT_DIR/registries/axis.axreg")
fi

# Validate and convert registry files to absolute paths
REGISTRY_ARGS=()
for reg in "${REGISTRY_FILES[@]}"; do
    if [[ ! -f "$reg" ]]; then
        echo "Error: Registry file not found: $reg"
        exit 1
    fi
    # Convert to absolute path
    if [[ "$reg" == /* ]]; then
        REGISTRY_ARGS+=("$reg")
    else
        REGISTRY_ARGS+=("$(cd "$(dirname "$reg")" && pwd)/$(basename "$reg")")
    fi
done

# Create coreir output directory if it doesn't exist
COREIR_DIR="$SCRIPT_DIR/coreir"
mkdir -p "$COREIR_DIR"

# Set up intermediate file paths
BASENAME_ONLY=$(basename "${VALIDATED_SOURCES[0]}" .ax)
COREIR_FILE="$COREIR_DIR/${BASENAME_ONLY}.coreir"

echo "=== Axis Compiler ==="
if [[ ${#VALIDATED_SOURCES[@]} -eq 1 ]]; then
    echo "Source:   ${VALIDATED_SOURCES[0]}"
else
    echo "Sources:  ${VALIDATED_SOURCES[0]}"
    for ((i=1; i<${#VALIDATED_SOURCES[@]}; i++)); do
        echo "          ${VALIDATED_SOURCES[$i]}"
    done
fi
echo "Output:   $OUTPUT_BINARY"
echo ""

# Step 1: Build the compiler if needed
echo "Step 1/3: Building compiler components..."
COMPILER_BIN="$SCRIPT_DIR/core-compiler/target/release/axis-compiler"
BRIDGE_BIN="$SCRIPT_DIR/rust-bridge/target/release/axis-rust-bridge"

if [[ ! -f "$COMPILER_BIN" ]]; then
    echo "  Building axis-compiler (first time only)..."
    cd "$SCRIPT_DIR/core-compiler"
    cargo build --release --quiet
fi

if [[ ! -f "$BRIDGE_BIN" ]]; then
    echo "  Building axis-rust-bridge (first time only)..."
    cd "$SCRIPT_DIR/rust-bridge"
    cargo build --release --quiet
fi

echo "  ✓ Compiler ready"
echo ""

# Step 2: Compile Axis source to Core IR
if [[ ${#SOURCE_ABS_LIST[@]} -eq 1 ]]; then
    echo "Step 2/3: Compiling source to Core IR..."
else
    echo "Step 2/3: Compiling ${#SOURCE_ABS_LIST[@]} source files to Core IR..."
fi
if ! "$COMPILER_BIN" --sources "${SOURCE_ABS_LIST[@]}" --registries "${REGISTRY_ARGS[@]}" --out "$COREIR_FILE"; then
    echo "Error: Compilation to Core IR failed"
    exit 1
fi
echo "  ✓ Core IR generated: $COREIR_FILE"
echo ""

# Step 3: Generate and build Rust executable
echo "Step 3/3: Generating executable..."
if ! "$BRIDGE_BIN" build "$COREIR_FILE" --out "$OUTPUT_BINARY"; then
    echo "Error: Executable generation failed"
    exit 1
fi

echo "  ✓ Executable created: $OUTPUT_BINARY"
echo ""
echo "=== Compilation successful! ==="
echo ""
# Show appropriate run command
if [[ "$OUTPUT_BINARY" == /* ]]; then
    echo "Run your program with: $OUTPUT_BINARY"
else
    echo "Run your program with: ./$OUTPUT_BINARY"
fi