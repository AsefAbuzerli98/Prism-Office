declare module "hyphen/en-us" {
  export function hyphenateSync(text: string, options?: { hyphenChar?: string; minWordLength?: number }): string;
}
declare module "hyphen/tr" {
  export function hyphenateSync(text: string, options?: { hyphenChar?: string; minWordLength?: number }): string;
}
