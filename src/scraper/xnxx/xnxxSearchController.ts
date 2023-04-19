import { load } from "cheerio";
import LustPress from "../../LustPress";
import c from "../../utils/options";
import { ISearchVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  try {
    const res = await lust.fetchBody(url);
    const $ = load(res);

    class PornhubSearch {
      search: object[];
      constructor() {
        this.search = $("div.mozaique > div")
          .map((i, el) => {
            return {
              link: `${c.XNXX}${$(el).find("a").attr("href")}`,
              // remove first "/" and last "/"
              id: $(el).find("a").attr("href")?.slice(1, -1),
              title: $(el).find("div.thumb-under").text().split("\n")
                .map((el) => el.trim()).filter((el) => el !== "")[0],
              image: $(el).find("img").attr("data-src"),
              duration: $(el).find("div.thumb-under").text().split("\n")
                .map((el) => el.trim()).filter((el) => el !== "")[2],
              rating: $(el).find("div.thumb-under").text().split("\n")
                .map((el) => el.trim()).filter((el) => el !== "")[1],
              video: `${c.XNXX}/embedframe/${$(el).find("img").attr("data-videoid")}`
            };
          }).get();
      }
    }
    
    const x = new PornhubSearch();
    if (x.search.length === 0) throw Error("No result found");
    const data = x.search as unknown as string[];
    const result: ISearchVideoData = {
      success: true,
      data: data,
      source: url,
    };
    return result;

  } catch (err) {
    const e = err as Error;
    throw Error(e.message);
  }
}