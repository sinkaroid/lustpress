import { URL } from "node:url";
import Keyv from "keyv";
import KeyvRedis from "@keyv/redis";
import pkg from "../package.json";
import { solveChallenge } from "./utils/ph-solver";

const keyv = process.env.REDIS_URL
  ? new Keyv({ store: new KeyvRedis(process.env.REDIS_URL) })
  : new Keyv();

keyv.on("error", (err) => console.log("Connection Error", err));
const ttl = 1000 * 60 * 60 * Number(process.env.EXPIRE_CACHE || "1");

const GEO_TIMEOUT_MS = 3000;
let cachedLastLocation: string | null = null;
let lastLocationTimestamp = 0;

function cachedLocationOrUnknown(): string {
  if (cachedLastLocation && Date.now() - lastLocationTimestamp < 3600_000) {
    return cachedLastLocation;
  }
  return "Unknown, Unknown";
}

class LustPress {
  url: string;
  useragent: string;
  private browserUA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36";
  private cookieCache: { [domain: string]: string } = {};

  constructor() {
    this.url = "";
    this.useragent = `${pkg.name}/${pkg.version} Bun/${Bun.version}`;
  }

  async getPornhubCookies(): Promise<string> {
    const baseUrl = "https://www.pornhub.com/";
    const headers = {
      "User-Agent": this.browserUA,
      "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
      "Accept-Language": "en-US,en;q=0.9",
      "Connection": "keep-alive",
      "Upgrade-Insecure-Requests": "1"
    };

    console.log("Fetching Pornhub Index for cookies...");
    let res = await fetch(baseUrl, { headers, redirect: "follow" });
    
    // Get cookies from set-cookie headers
    const initialCookies = res.headers.getSetCookie().map(c => c.split(";")[0]).join("; ");
    let text = await res.text();

    if (text.includes("leastFactor")) {
      console.log("Pornhub Challenge detected at Index. Solving...");
      const challengeCookie = await solveChallenge(text);
      const combinedCookies = [initialCookies, challengeCookie, "age_verified=1"].filter(Boolean).join("; ");
      
      // Verify cookies
      console.log("Verifying cookies at Index...");
      res = await fetch(baseUrl, { 
        headers: { ...headers, Cookie: combinedCookies },
        redirect: "follow"
      });
      text = await res.text();
      
      if (!text.includes("leastFactor")) {
        console.log("Pornhub cookies verified!");
        return combinedCookies;
      } else {
        console.log("Warning: Challenge still present after solve attempt.");
        return combinedCookies;
      }
    }
    
    return initialCookies || "age_verified=1";
  }

  async fetchBody(url: string): Promise<Buffer> {
    const cached = await keyv.get(url);
    if (cached) {
      console.log(`[CACHE HIT] ${url}`);
      return cached;
    }

    const isPornhub = /pornhub\.com/i.test(url);
    const domain = new URL(url).hostname;

    const headers: Record<string, string> = {
      "User-Agent": isPornhub ? this.browserUA : this.useragent,
    };

    if (isPornhub) {
      if (!this.cookieCache[domain]) {
        this.cookieCache[domain] = await this.getPornhubCookies();
      }
      headers["Cookie"] = this.cookieCache[domain];
      headers["Referer"] = "https://www.pornhub.com/";
      headers["Accept"] = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8";
      headers["Accept-Language"] = "en-US,en;q=0.9";
    }

    console.log(`[FETCH] ${url}`);
    let res = await fetch(url, { headers, redirect: "follow" });
    let text = await res.text();

    if (isPornhub && text.includes("leastFactor")) {
      console.log("Challenge detected at target URL. Re-authenticating...");
      this.cookieCache[domain] = await this.getPornhubCookies();
      res = await fetch(url, { 
        headers: { ...headers, Cookie: this.cookieCache[domain] },
        redirect: "follow" 
      });
      text = await res.text();
    }

    const body = Buffer.from(text);
    if (!url.includes("/random")) await keyv.set(url, body, ttl);
    return body;
  }

  // Utility methods
  removeHtmlTag(str: string): string {
    return str.replace(/(\r\n|\n|\r)/gm, "").replace(/\s+/g, "");
  }

  removeHtmlTagWithoutSpace(str: string): string {
    return str.replace(/(\r\n|\n|\r|\t)/gm, "").replace(/\\/g, "").replace(/\s+/g, " ").trim();
  }

  removeAllSingleQuoteOnArray(arr: string[]): string[] {
    return arr.map((item) => item.replace(/'/g, ""));
  }

  timeAgo(input: Date) {
    const date = new Date(input);
    const formatter = new Intl.RelativeTimeFormat("en");
    const ranges: { [key: string]: number } = {
      years: 3600 * 24 * 365,
      months: 3600 * 24 * 30,
      weeks: 3600 * 24 * 7,
      days: 3600 * 24,
      hours: 3600,
      minutes: 60,
      seconds: 1,
    };
    const secondsElapsed = (date.getTime() - Date.now()) / 1000;
    for (const key in ranges) {
      if (ranges[key] < Math.abs(secondsElapsed)) {
        const delta = secondsElapsed / ranges[key];
        return formatter.format(Math.round(delta), key as Intl.RelativeTimeFormatUnit);
      }
    }
  }

  secondToMinute(seconds: number): string {
    const minutes = Math.floor(seconds / 60);
    const second = seconds % 60;
    return `${minutes}min, ${second}sec`;
  }

  currentProccess() {
    const rss = process.memoryUsage().rss / 1024 / 1024;
    const heap = process.memoryUsage().heapUsed / 1024 / 1024;
    const heaptotal = process.memoryUsage().heapTotal / 1024 / 1024;
    return {
      rss: `${Math.round(rss * 100) / 100} MB`,
      heap: `${Math.round(heap * 100) / 100}/${Math.round(heaptotal * 100) / 100} MB`,
    };
  }

  async getServer(): Promise<string> {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), GEO_TIMEOUT_MS);

    try {
      // ip-api free tier often rejects HTTPS requests with 403;
      const raw = await fetch("https://ipwho.is/", {
        signal: controller.signal,
      });
      if (!raw.ok) {
        return cachedLocationOrUnknown();
      }
      const data = await raw.json() as {
        success?: boolean;
        country?: string;
        region?: string;
      };
      if (data.success === false) {
        return cachedLocationOrUnknown();
      }
      const country = data.country?.trim();
      const region = data.region?.trim();
      if (!country || !region) {
        return cachedLocationOrUnknown();
      }

      const location = `${country}, ${region}`;
      cachedLastLocation = location;
      lastLocationTimestamp = Date.now();
      return location;
    } catch {
      return cachedLocationOrUnknown();
    } finally {
      clearTimeout(timeoutId);
      if (!controller.signal.aborted) {
        controller.abort();
      }
    }
  }
}

export const lust = new LustPress();
export default LustPress;
