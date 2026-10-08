/** Only source-authored GitHub commit URLs make personal scoring optional. */
export function isCommitArticle(url: string): boolean {
  return /^https?:\/\/github\.com\/[^/]+\/[^/]+\/commits?\//.test(url);
}

export function commitProject(url: string): string | null {
  const match =
    /^https?:\/\/github\.com\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)\/commits?\/[^/?#]+/.exec(
      url,
    );
  return match ? `${match[1]}/${match[2]}` : null;
}
