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
      data: object;
      constructor() {
        this.search = $("div.wrap")
          .map((i, el) => {
            const link = $(el).find("a").attr("href");
            const id = link?.split("=")[1];
            const title = $(el).find("a").attr("title");
            const image = $(el).find("img").attr("src");
            const duration = $(el).find("var.duration").text();
            const views = $(el).find("div.videoDetailsBlock").find("span.views").text();
            return {
              link: `${c.PORNHUB}${link}`,
              id: id,
              title: title,
              image: image,
              duration: duration,
              views: views,
              video: `${c.PORNHUB}/embed/${id}`,
            };
          }).get();

        this.data = this.search.filter((el: any) => {
          return el.link.includes("javascript:void(0)") === false && el.image?.startsWith("data:image") === false;
        });
      }

    }
    
    const ph = new PornhubSearch();
    if (ph.search.length === 0) throw Error("No result found");
    const data = ph.data as string[];
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