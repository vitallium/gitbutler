import { branchPrNumberUpdate } from "$components/forge/prNumberSync";
import { describe, expect, test } from "vitest";

describe("branchPrNumberUpdate", () => {
	test("builds a branch review metadata update for a listed review", () => {
		expect(
			branchPrNumberUpdate({
				projectId: "project-1",
				stackId: "stack-1",
				branchName: "feature/login",
				canListReviews: true,
				listedReviewNumber: 12,
			}),
		).toEqual({
			projectId: "project-1",
			stackId: "stack-1",
			branchName: "feature/login",
			prNumber: 12,
		});
	});

	test("does not update when review listing is unavailable", () => {
		expect(
			branchPrNumberUpdate({
				projectId: "project-1",
				stackId: "stack-1",
				branchName: "feature/login",
				canListReviews: false,
				listedReviewNumber: 12,
			}),
		).toBeUndefined();
	});

	test("does not update when exact source-branch lookup found no review", () => {
		expect(
			branchPrNumberUpdate({
				projectId: "project-1",
				stackId: "stack-1",
				branchName: "feature/login",
				canListReviews: true,
				listedReviewNumber: undefined,
			}),
		).toBeUndefined();
	});
});
