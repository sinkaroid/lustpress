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
      links: string[];
      ids: string[];
      titles: string[];
      images: string[];
      durations: string[];
      views: string[];
      search: object[];

      constructor() {
        const cards = $("div.video-box.pc");

        this.links = cards
          .map((i, el) => {
            return $(el).find("a.tm_video_link").attr("href");
          })
          .get();

        this.ids = this.links.map((link) => link?.split("/")[2]);

        this.titles = cards
          .map((i, el) => {
            return $(el).find("a.tm_video_title span").text().trim();
          })
          .get();

        this.images = cards
          .map((i, el) => {
            return (
              $(el).find("img.thumb-image").attr("data-src") ||
              $(el).find("img.thumb-image").attr("src")
            );
          })
          .get();

        this.durations = cards
          .map((i, el) => {
            return $(el).find("div.tm_video_duration span").text().trim();
          })
          .get();

        this.views = cards
          .map((i, el) => {
            return (
              $(el)
                .find(".view-rating-container .info-views")
                .first()
                .text()
                .trim() || "None"
            );
          })
          .get();

        this.search = cards
          .map((i, el) => {
            return {
              link: `${c.YOUPORN}${this.links[i]}`,
              id: this.ids[i],
              title: lust.removeHtmlTagWithoutSpace(this.titles[i]),
              image: this.images[i],
              duration: this.durations[i],
              views: this.views[i],
              video: `https://www.youporn.com/embed/${this.ids[i]}`,
            };
          })
          .get();
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
