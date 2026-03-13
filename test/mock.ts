import test from "node:test";
import assert from "node:assert/strict";

import p from "phin";
import { chromium } from "playwright";
import { load } from "cheerio";
import pkg from "../package.json";

const url = "https://www.pornhub.com/view_video.php?viewkey=697a92abd524b";

async function getCookies() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();

  const page = await context.newPage();
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await page.waitForLoadState("networkidle");

  const cookies = await context.cookies();
  await browser.close();

  return cookies.map(c => `${c.name}=${c.value}`).join("; ");
}

test("pornhub og:title extraction", async () => {

  const cookieHeader = await getCookies();

  const res = await p({
    url,
    headers: {
      "User-Agent": process.env.USER_AGENT || `${pkg.name}/${pkg.version} Node.js/${process.versions.node}`,
      "Cookie": cookieHeader
    },
    followRedirects: true
  });

  console.log("Cookie Header:", cookieHeader);
  console.log("Status:", res.statusCode);

  const html = res.body.toString();
  // console.log(html.slice(0, 1000));

  const $ = load(html);
  const title = $("meta[property='og:title']").attr("content");

  console.log("Title:", title);

  assert.ok(title, "og:title should exist");
});