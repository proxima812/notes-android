import { describe, expect, it } from "vitest";

import { showsItsOwnAddress } from "./linkRanges";

describe("showsItsOwnAddress", () => {
  it("recognises a link that is still just its address", () => {
    expect(showsItsOwnAddress("https://workos.com", "https://workos.com")).toBe(true);
  });

  it("ignores the parts a person does not read as part of the address", () => {
    expect(showsItsOwnAddress("workos.com", "https://www.workos.com/")).toBe(true);
    expect(showsItsOwnAddress("WorkOS.com/", "http://workos.com")).toBe(true);
  });

  /// The guard that keeps somebody's own words from being overwritten.
  it("leaves a link that already carries a name", () => {
    expect(showsItsOwnAddress("WorkOS — Enterprise Ready", "https://workos.com")).toBe(false);
    expect(showsItsOwnAddress("тут", "https://workos.com")).toBe(false);
  });

  it("does not treat one address as another", () => {
    expect(showsItsOwnAddress("workos.com", "https://clerk.com")).toBe(false);
    expect(showsItsOwnAddress("workos.com/pricing", "https://workos.com")).toBe(false);
  });

  it("says no to empty text rather than renaming nothing", () => {
    expect(showsItsOwnAddress("", "https://workos.com")).toBe(false);
    expect(showsItsOwnAddress("  ", "https://workos.com")).toBe(false);
  });
});
