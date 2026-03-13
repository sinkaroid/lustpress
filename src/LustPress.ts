import { URL } from "node:url";
import { chromium } from "playwright";
import p, { IResponse } from "phin";
import Keyv from "keyv";
import KeyvRedis from "@keyv/redis";
import pkg from "../package.json";

const keyv = process.env.REDIS_URL
  ? new Keyv({ store: new KeyvRedis(process.env.REDIS_URL) })
  : new Keyv();

keyv.on("error", err => console.log("Connection Error", err));
const ttl = 1000 * 60 * 60 * Number(process.env.EXPIRE_CACHE);

class LustPress {

  url: string;
  useragent: string;
  private cookieCache: { [domain: string]: string } = {};


  constructor() {
    this.url = "";
    this.useragent = `${pkg.name}/${pkg.version} Node.js/${process.versions.node}`;
  }

  async getCookies(url: string): Promise<string> {
    const browser = await chromium.launch({ headless: true });

    const context = await browser.newContext({
      userAgent: process.env.USER_AGENT || `${pkg.name}/${pkg.version} Node.js/${process.versions.node}`
    });

    const page = await context.newPage();
    await page.goto(url, { waitUntil: "domcontentloaded" });
    await page.waitForLoadState("networkidle");
    const cookies = await context.cookies();
    await browser.close();
    return cookies.map(c => `${c.name}=${c.value}`).join("; ");
  }

  /**
   * Fetch body from url and check if it's cached
   * @param url url to fetch
   * @returns Buffer 
   */
  async fetchBody(url: string): Promise<Buffer> {

    const cached = await keyv.get(url);

    if (cached) {
      console.log("Fetching from cache");
      return cached;

    } else if (url.includes("/random")) {

      console.log("Random should not be cached");

      const isPornhub = /pornhub\.com/i.test(url);

      let cookieHeader = "";

      if (isPornhub) {
        const domain = new URL(url).hostname;

        if (this.cookieCache[domain]) {
          console.log("Using cached cookie");
          cookieHeader = this.cookieCache[domain];
        } else {
          console.log("Solving challenge via playwright");
          cookieHeader = await this.getCookies(url);
          this.cookieCache[domain] = cookieHeader;
        }
      }

      console.log(`Fetching from ${isPornhub ? "phin (cookie)" : "phin"}`);

      const res = await p({
        url: url,
        headers: {
          "User-Agent": process.env.USER_AGENT || `${pkg.name}/${pkg.version} Node.js/${process.versions.node}`,
          ...(cookieHeader && { "Cookie": cookieHeader })
        },
        followRedirects: true
      });

      if (isPornhub && res.statusCode !== 200) {
        const domain = new URL(url).hostname;
        console.log("Cookie invalid, clearing cache and retrying via playwright");
        delete this.cookieCache[domain];
        const newCookie = await this.getCookies(url);
        this.cookieCache[domain] = newCookie;
        const retry = await p({
          url: url,
          headers: {
            "User-Agent": process.env.USER_AGENT || `${pkg.name}/${pkg.version} Node.js/${process.versions.node}`,
            "Cookie": newCookie
          },
          followRedirects: true
        });
        return retry.body;
      }

      return res.body;

    } else {

      console.log("Fetching from source");
      url = url.replace(/\/\//g, "/");
      const isPornhub = /pornhub\.com/i.test(url);

      let cookieHeader = "";

      if (isPornhub) {
        const domain = new URL(url).hostname;

        if (this.cookieCache[domain]) {
          console.log("Using cached cookie");
          cookieHeader = this.cookieCache[domain];
        } else {
          console.log("Solving challenge via playwright");
          cookieHeader = await this.getCookies(url);
          this.cookieCache[domain] = cookieHeader;
        }
      }

      console.log(`Fetching from ${isPornhub ? "phin (cookie)" : "phin"}`);

      const res = await p({
        url: url,
        headers: {
          "User-Agent": process.env.USER_AGENT || `${pkg.name}/${pkg.version} Node.js/16.9.1`,
          ...(cookieHeader && { "Cookie": cookieHeader })
        },
        followRedirects: true
      });

      if (isPornhub && res.statusCode !== 200) {
        const domain = new URL(url).hostname;
        console.log("Cookie invalid, clearing cache and retrying via playwright");
        delete this.cookieCache[domain];
        const newCookie = await this.getCookies(url);
        this.cookieCache[domain] = newCookie;
        const retry = await p({
          url: url,
          headers: {
            "User-Agent": process.env.USER_AGENT || `${pkg.name}/${pkg.version} Node.js/16.9.1`,
            "Cookie": newCookie
          },
          followRedirects: true
        });
        return retry.body;
      }

      await keyv.set(url, res.body, ttl);

      return res.body;
    }
  }

  /**
   * remove html tag and bunch of space
   * @param str string to remove html tag
   * @returns string
   */
  removeHtmlTag(str: string): string {
    str = str.replace(/(\r\n|\n|\r)/gm, "");
    str = str.replace(/\s+/g, "");
    return str;
  }

  /**
   * remove html tag without space
   * @param str string to remove html tag
   * @returns string
   */
  removeHtmlTagWithoutSpace(str: string): string {
    str = str.replace(/(\r\n|\n|\r|\t)/gm, "");
    str = str.replace(/\\/g, "");
    str = str.replace(/\s+/g, " ");
    return str.trim();
  }

  /**
   * remove all single quote on array
   * @param arr array to remove single quote
   * @returns string[]
   */
  removeAllSingleQuoteOnArray(arr: string[]): string[] {
    return arr.map((item) => item.replace(/'/g, ""));
  }

  /**
   * time ago converter
   * @param input date to convert
   * @returns string
   */
  timeAgo(input: Date) {
    const date = new Date(input);
    const formatter: any = new Intl.RelativeTimeFormat("en");
    const ranges: { [key: string]: number } = {
      years: 3600 * 24 * 365,
      months: 3600 * 24 * 30,
      weeks: 3600 * 24 * 7,
      days: 3600 * 24,
      hours: 3600,
      minutes: 60,
      seconds: 1
    };
    const secondsElapsed = (date.getTime() - Date.now()) / 1000;
    for (const key in ranges) {
      if (ranges[key] < Math.abs(secondsElapsed)) {
        const delta = secondsElapsed / ranges[key];
        return formatter.format(Math.round(delta), key);
      }
    }
  }

  /**
   * convert seconds to minute
   * @param seconds seconds to convert
   * @returns string
   */
  secondToMinute(seconds: number): string {
    const minutes = Math.floor(seconds / 60);
    const second = seconds % 60;
    return `${minutes}min, ${second}sec`;
  }

  /**
   * get current process memory usage
   * @returns object
   */
  currentProccess() {
    const arr = [1, 2, 3, 4, 5, 6, 9, 7, 8, 9, 10];
    arr.reverse();
    const rss = process.memoryUsage().rss / 1024 / 1024;
    const heap = process.memoryUsage().heapUsed / 1024 / 1024;
    const heaptotal = process.memoryUsage().heapTotal / 1024 / 1024;
    return {
      rss: `${Math.round(rss * 100) / 100} MB`,
      heap: `${Math.round(heap * 100) / 100}/${Math.round(heaptotal * 100) / 100} MB`
    };
  }

  /**
   * fetch this server location
   * @returns <Promise<string>>
   */
  async getServer(): Promise<string> {
    const raw = await p({
      "url": "http://ip-api.com/json",
      "parse": "json"
    }) as IResponse;
    const data = raw.body as unknown as { country: string, regionName: string };
    return `${data.country}, ${data.regionName}`;
  }
}

export default LustPress;
