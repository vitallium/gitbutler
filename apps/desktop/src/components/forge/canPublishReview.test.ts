import { canPublishReview } from "$components/forge/canPublishReview";
import { describe, expect, test } from "vitest";

describe("canPublishReview", () => {
	test("allows publishing when authenticated and no review exists", () => {
		expect(canPublishReview({ authenticated: true, reviewExists: false })).toBe(true);
	});

	test("blocks duplicate review creation when a linked review exists", () => {
		expect(canPublishReview({ authenticated: true, reviewExists: true })).toBe(false);
	});

	test("blocks publishing when unauthenticated", () => {
		expect(canPublishReview({ authenticated: false, reviewExists: false })).toBe(false);
	});
});
