export function canPublishReview(args: { authenticated: boolean; reviewExists: boolean }) {
	return args.authenticated && !args.reviewExists;
}
