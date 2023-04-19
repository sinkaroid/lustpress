import { load } from "cheerio";
import LustPress from "../../LustPress";
import c from "../../utils/options";
import { ISearchVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  try {
    const res = await lust.fetchBody(url);
    const $ = load(res);

    class YouPornSearch {
      dur: string[];
      search: object[];
      constructor() {
        this.dur = $("div.video-duration").map((i, el) => {
          return $(el).text();
        }).get();
        this.search = $("a[href^='/watch/']")
          .map((i, el) => {
            const link = $(el).attr("href");
            const id = `${link}`.split("/")[2] + "/" + `${link}`.split("/")[3];
            const title = $(el).find("div.video-box-title").text();
            const image = $(el).find("img").attr("data-thumbnail");
            return {
              link: `${c.YOUPORN}${link}`,
              id: id,
              title: lust.removeHtmlTagWithoutSpace(title),
              image: image,
              duration: this.dur[i],
              views: "None",
              video: `https://www.youporn.com/embed/${id}`,
            };
          }).get();
      }
    }
    
    const yp = new YouPornSearch();
    if (yp.search.length === 0) throw Error("No result found");
    const data = yp.search as unknown as string[];
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