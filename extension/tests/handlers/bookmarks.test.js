/**
 * Tests: bookmark handlers (bookmarks_list, bookmarks_create, bookmarks_update,
 * bookmarks_remove, bookmarks_move, bookmarks_search).
 */
import { describe, it, expect, beforeAll } from "vitest";
import { getBrowser } from "../setup.js";

let handlers = {};
beforeAll(async () => {
  handlers = (await import("../../src/background.js")).handlers;
});

function bmNode(id, overrides = {}) {
  return { id, title: `Node ${id}`, url: id % 2 ? `https://site${id}.com` : undefined, parentId: "toolbar_____", ...overrides };
}

// ── bookmarks_list ───────────────────────────────────────────────────────

describe("bookmarks_list", () => {
  it("returns full tree when no folder_id", async () => {
    const browser = getBrowser();
    browser.bookmarks.getTree.mockResolvedValue([bmNode(0, { title: "Root" })]);
    const result = await handlers.bookmarks_list({});
    expect(result).toHaveLength(1);
    expect(browser.bookmarks.getTree).toHaveBeenCalled();
    expect(browser.bookmarks.getSubTree).not.toHaveBeenCalled();
  });
  it("returns subtree when folder_id given", async () => {
    const browser = getBrowser();
    browser.bookmarks.getSubTree.mockResolvedValue([bmNode("abc")]);
    const result = await handlers.bookmarks_list({ folder_id: "abc" });
    expect(result).toHaveLength(1);
    expect(browser.bookmarks.getSubTree).toHaveBeenCalledWith("abc");
  });
});

// ── bookmarks_create ─────────────────────────────────────────────────────

describe("bookmarks_create", () => {
  it("throws when title missing", async () => {
    await expect(handlers.bookmarks_create({ url: "https://x.com" })).rejects.toThrow("title required");
  });
  it("creates bookmark with title only (folder)", async () => {
    const browser = getBrowser();
    browser.bookmarks.create.mockResolvedValue(bmNode("f1"));
    const result = await handlers.bookmarks_create({ title: "Folder" });
    expect(browser.bookmarks.create).toHaveBeenCalledWith({ title: "Folder" });
    expect(result.id).toBe("f1");
  });
  it("creates bookmark with url + parent_id + index", async () => {
    const browser = getBrowser();
    browser.bookmarks.create.mockResolvedValue(bmNode("b1", { url: "https://zen.org" }));
    await handlers.bookmarks_create({ title: "Zen", url: "https://zen.org", parent_id: "p99", index: 3 });
    expect(browser.bookmarks.create).toHaveBeenCalledWith({
      title: "Zen", url: "https://zen.org", parentId: "p99", index: 3,
    });
  });
});

// ── bookmarks_update ─────────────────────────────────────────────────────

describe("bookmarks_update", () => {
  it("throws when id missing", async () => {
    await expect(handlers.bookmarks_update({ title: "New" })).rejects.toThrow("id required");
  });
  it("updates title", async () => {
    const browser = getBrowser();
    browser.bookmarks.update.mockResolvedValue(bmNode("b1", { title: "New Title" }));
    await handlers.bookmarks_update({ id: "b1", title: "New Title" });
    expect(browser.bookmarks.update).toHaveBeenCalledWith("b1", { title: "New Title" });
  });
  it("updates url", async () => {
    const browser = getBrowser();
    await handlers.bookmarks_update({ id: "b2", url: "https://new.com" });
    expect(browser.bookmarks.update).toHaveBeenCalledWith("b2", { url: "https://new.com" });
  });
});

// ── bookmarks_remove ─────────────────────────────────────────────────────

describe("bookmarks_remove", () => {
  it("throws when id missing", async () => {
    await expect(handlers.bookmarks_remove({})).rejects.toThrow("id required");
  });
  it("removes single bookmark (non-recursive)", async () => {
    const browser = getBrowser();
    const result = await handlers.bookmarks_remove({ id: "b1" });
    expect(browser.bookmarks.remove).toHaveBeenCalledWith("b1");
    expect(browser.bookmarks.removeTree).not.toHaveBeenCalled();
    expect(result).toEqual({ removed: "b1" });
  });
  it("removes tree when recursive", async () => {
    const browser = getBrowser();
    await handlers.bookmarks_remove({ id: "folder1", recursive: true });
    expect(browser.bookmarks.removeTree).toHaveBeenCalledWith("folder1");
    expect(browser.bookmarks.remove).not.toHaveBeenCalled();
  });
});

// ── bookmarks_move ───────────────────────────────────────────────────────

describe("bookmarks_move", () => {
  it("throws when id missing", async () => {
    await expect(handlers.bookmarks_move({ parent_id: "p1" })).rejects.toThrow("id required");
  });
  it("moves with parent_id only", async () => {
    const browser = getBrowser();
    browser.bookmarks.move.mockResolvedValue(bmNode("b1"));
    await handlers.bookmarks_move({ id: "b1", parent_id: "parent" });
    expect(browser.bookmarks.move).toHaveBeenCalledWith("b1", { parentId: "parent" });
  });
  it("moves with index only", async () => {
    const browser = getBrowser();
    await handlers.bookmarks_move({ id: "b2", index: 5 });
    expect(browser.bookmarks.move).toHaveBeenCalledWith("b2", { index: 5 });
  });
  it("moves with parent_id + index", async () => {
    const browser = getBrowser();
    await handlers.bookmarks_move({ id: "b3", parent_id: "toolbar", index: 0 });
    expect(browser.bookmarks.move).toHaveBeenCalledWith("b3", {
      parentId: "toolbar", index: 0,
    });
  });
});

// ── bookmarks_search ─────────────────────────────────────────────────────

describe("bookmarks_search", () => {
  it("throws when query missing", async () => {
    await expect(handlers.bookmarks_search({})).rejects.toThrow("query required");
  });
  it("searches and returns results", async () => {
    const browser = getBrowser();
    browser.bookmarks.search.mockResolvedValue([bmNode(1, { title: "GitHub" })]);
    const result = await handlers.bookmarks_search({ query: "github" });
    expect(browser.bookmarks.search).toHaveBeenCalledWith("github");
    expect(result).toHaveLength(1);
  });
});
