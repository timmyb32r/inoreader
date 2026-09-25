import { defineConfig, devices } from "@playwright/test";

const baseURL=process.env.READER_ACCEPTANCE_URL;
if(!baseURL)throw new Error("READER_ACCEPTANCE_URL is required; real-backend acceptance must never be skipped");

export default defineConfig({
  testDir:"./e2e",testMatch:"**/real-backend.acceptance.spec.ts",timeout:60_000,retries:0,
  reporter:[["html",{open:"never",outputFolder:"playwright-report-acceptance"}],["list"]],
  use:{baseURL,trace:"retain-on-failure",screenshot:"only-on-failure"},
  projects:[{name:"real-backend-ydb",use:{...devices["Desktop Chrome"]}}],
});
