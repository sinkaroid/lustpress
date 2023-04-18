import { load } from "cheerio";
import LustPress from "../../LustPress";
import c from "../../utils/options";
import { ISearchVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  try {
    const res = await lust.fetchBody(url);
    const $ = load(res);

    class RedTubeSearch {
      views: string[];
      search: object[];
      data: object;
      constructor() {
        this.views = $("span.video_count")
          .map((i, el) => {
            const views = $(el).text();
            return views;
          }).get();
        this.search = $("a.video_link")
          .map((i, el) => {
            const link = $(el).attr("href");
            const id = link?.split("/")[1];
            const title = $(el).find("img").attr("alt");
            const image = $(el).find("img").attr("data-src");
            const duration = $(el).find("span.duration").text().split(" ").map((el: string) => {
              return el.replace(/[^0-9:]/g, "");
            }).filter((el: string) => {
              return el.includes(":");
            }).join(" ");

            return {
              link: `${c.REDTUBE}${link}`,
              id: id,
              title: title,
              image: image,
              duration: duration,
              views: this.views[i],
              video: `https://embed.redtube.com/?id=${id}`,
                
            };
          }).get();
            

          

        this.data = this.search.filter((el: any) => {
          return el.link.includes("javascript:void(0)") === false && el.image?.startsWith("data:image") === false;
        });
      }

    }
    
    const red = new RedTubeSearch();

    if (red.search.length === 0) throw Error("No result found");
    const data = red.data as string[];
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