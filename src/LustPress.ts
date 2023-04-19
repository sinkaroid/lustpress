import p, { IResponse } from "phin";
import Keyv from "keyv";
import pkg from "../package.json";


const keyv = new Keyv(process.env.REDIS_URL);

keyv.on("error", err => console.log("Connection Error", err));
const ttl = 1000 * 60 * 60 * Number(process.env.EXPIRE_CACHE);


class LustPress {
  url: string;
  useragent: string;
  constructor() {
    this.url = "";
    this.useragent = `${pkg.name}/${pkg.version} Node.js/16.9.1`;
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
      const res = await p({ 
        url: url,
        "headers": {
          "User-Agent": process.env.USER_AGENT || `${pkg.name}/${pkg.version} Node.js/16.9.1`,
        }, 
        followRedirects: true
      });
      return res.body;
    } else {
      console.log("Fetching from source");
      url = url.replace(/\/\//g, "/");
      const res = await p({ 
        url: url,
        "headers": {
          "User-Agent": process.env.USER_AGENT || `${pkg.name}/${pkg.version} Node.js/16.9.1`,
        },
        followRedirects: true
      });
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
