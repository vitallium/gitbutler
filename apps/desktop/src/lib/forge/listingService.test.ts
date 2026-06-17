import {
	forgeReviewsToPullRequestState,
	selectPullRequestBySourceBranch,
} from "$lib/forge/listingService.svelte";
import { describe, expect, test } from "vitest";
import type { ForgeReview } from "$lib/forge/interface/types";

function forgeReview(overrides: Partial<ForgeReview> = {}): ForgeReview {
	return {
		htmlUrl: "https://gitlab.com/gitbutler/gitbutler/-/merge_requests/12",
		number: 12,
		title: "Add login",
		body: null,
		author: null,
		labels: [],
		draft: false,
		sourceBranch: "feature/login",
		targetBranch: "main",
		sha: "deadbeef",
		createdAt: "2026-06-17T00:00:00Z",
		modifiedAt: "2026-06-17T00:00:00Z",
		mergedAt: null,
		closedAt: null,
		repositorySshUrl: null,
		repositoryHttpsUrl: null,
		repoOwner: null,
		reviewers: [],
		...overrides,
	};
}

describe("listingService review branch lookup", () => {
	test("selects reviews by exact source branch", () => {
		const state = forgeReviewsToPullRequestState([forgeReview()]);

		expect(selectPullRequestBySourceBranch(state, "feature/login")?.number).toBe(12);
	});

	test("does not fuzzy-match similar GitLab source branches", () => {
		const state = forgeReviewsToPullRequestState([
			forgeReview({ sourceBranch: "vitaly/feature-login" }),
		]);

		expect(selectPullRequestBySourceBranch(state, "feature/login")).toBeUndefined();
	});
});
