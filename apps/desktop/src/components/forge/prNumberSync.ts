export type BranchPrNumberUpdate = {
	projectId: string;
	stackId: string;
	branchName: string;
	prNumber: number;
};

export function branchPrNumberUpdate(args: {
	projectId: string;
	stackId: string;
	branchName: string;
	canListReviews: boolean;
	listedReviewNumber: number | undefined;
}): BranchPrNumberUpdate | undefined {
	if (!args.canListReviews || args.listedReviewNumber === undefined) return undefined;
	return {
		projectId: args.projectId,
		stackId: args.stackId,
		branchName: args.branchName,
		prNumber: args.listedReviewNumber,
	};
}
