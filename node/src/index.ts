import { readFile } from "node:fs/promises";
import { resolve as resolvePath } from "node:path";
import { createRequire } from "node:module";
import { TextDecoder } from "node:util";

/** A loaded, immutable snapshot of an fsman manifest. */
export interface FSManConfig {
  /** Absolute path from which the manifest was loaded. */
  readonly path: string;
  /** Manifest contents captured by {@link FSMan.load}. */
  readonly source: string;
}

export type ResolvedEntryType = "file" | "directory" | "symlink" | "other";

/** A filesystem entry selected by a manifest. */
export interface ResolvedEntry {
  readonly path: string;
  readonly type: ResolvedEntryType;
  readonly children: readonly ResolvedEntry[];
}

/** The hierarchical result of resolving a manifest. */
export interface ResolvedTree {
  readonly path: string;
  readonly type: "directory";
  readonly children: readonly ResolvedEntry[];
}

export interface ResolveOptions {
  /** Directory against which manifest paths are resolved. Defaults to `process.cwd()`. */
  readonly cwd?: string;
  /** Maximum number of levels traversed by each `**` or `***` directive. */
  readonly depth?: number;
  /** Collapse complete recursive selections into their highest selected directory. */
  readonly short?: boolean;
  /** Return a path array instead of a tree. */
  readonly flat?: boolean;
}

export interface FlatResolveOptions extends ResolveOptions {
  readonly flat: true;
}

interface NativeBinding {
  validate(source: string): Promise<boolean>;
  resolve(
    source: string,
    root: string,
    depth: number | null,
    short: boolean,
    flat: boolean,
  ): Promise<string>;
}

const native = createRequire(import.meta.url)("../index.node") as NativeBinding;

/** An error reported by the native fsman binding. */
export class FSManError extends Error {
  constructor(message: string, options: { cause?: unknown } = {}) {
    super(message, { cause: options.cause });
    this.name = "FSManError";
  }
}

/** Promise-based access to the fsman parser and filesystem resolver. */
export class FSMan {
  static #currentConfig: FSManConfig | undefined;

  /**
   * Load a manifest into memory. The returned config is a snapshot: later file
   * changes do not affect validation or resolution of this value.
   */
  static async load(path: string): Promise<FSManConfig> {
    const absolutePath = resolvePath(path);
    const contents = await readFile(absolutePath);
    const source = new TextDecoder("utf-8", { fatal: true }).decode(contents);
    const config = Object.freeze({ path: absolutePath, source });
    this.#currentConfig = config;
    return config;
  }

  /**
   * Validate a loaded manifest. When omitted, `config` defaults to the value
   * most recently returned by {@link FSMan.load}.
   */
  static async validate(config: FSManConfig = this.#requireCurrentConfig()): Promise<boolean> {
    return native.validate(config.source);
  }

  static async resolve(
    config: FSManConfig,
    options: FlatResolveOptions,
  ): Promise<string[]>;
  static async resolve(
    config: FSManConfig,
    options?: ResolveOptions & { readonly flat?: false },
  ): Promise<ResolvedTree>;
  static async resolve(
    config: FSManConfig,
    options: ResolveOptions = {},
  ): Promise<ResolvedTree | string[]> {
    validateDepth(options.depth);

    try {
      const result = await native.resolve(
        config.source,
        resolvePath(options.cwd ?? process.cwd()),
        options.depth ?? null,
        options.short ?? false,
        options.flat ?? false,
      );
      return JSON.parse(result) as ResolvedTree | string[];
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      throw new FSManError(message, { cause });
    }
  }

  static #requireCurrentConfig(): FSManConfig {
    if (this.#currentConfig === undefined) {
      throw new FSManError("no manifest is loaded; call FSMan.load() first");
    }
    return this.#currentConfig;
  }
}

function validateDepth(depth: number | undefined): void {
  if (depth !== undefined && (!Number.isSafeInteger(depth) || depth < 0)) {
    throw new RangeError("depth must be a non-negative safe integer");
  }
}
