import { load } from "cheerio";
import LustPress from "../../LustPress";
import c from "../../utils/options";
import { ISearchVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  try {
    const res = await lust.fetchBody(url);
    const $ = load(res);

    class XhamsterSearch {
      search: any;
      constructor() {
        const views = $("div.video-thumb-views")
          .map((i, el) => {
            const views = $(el).text();
            return views;
          }).get();
        const duration = $("span[data-role='video-duration']")
          .map((i, el) => {
            const duration = $(el).text();
            return duration;
          }).get();
        this.search = $("a.video-thumb__image-container")
          .map((i, el) => {
            const link = $(el).attr("href");

            return {
              link: `${link}`,
              id: link?.split("/")[3] + "/" + link?.split("/")[4],
              title: $(el).find("img").attr("alt"),
              image: $(el).find("img").attr("src"),
              duration: duration[i],
              views: views[i],
              video: `${c.XHAMSTER}/embed/${link?.split("-").pop()}`
            };
          }).get();    
      }
    }
    
    const xh = new XhamsterSearch();
    if (xh.search.length === 0) throw Error("No result found");
    const data = xh.search as unknown as string[];
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