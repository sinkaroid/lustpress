import { load } from "cheerio";
import LustPress from "../../LustPress";
import { IVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  try {
    const resolve = await lust.fetchBody(url);
    const $ = load(resolve);

    class RedTube { 
      link: string;
      id: string;
      title: string;
      image: string;
      duration: string;
      views: string;
      rating: string;
      publish: string;
      upVote: string;
      downVote: null;
      video: string;
      tags: string[];
      models: string[];
      constructor() {
        this.link = $("link[rel='canonical']").attr("href") || "None";
        this.id = this.link.split("/")[3] || "None";
        this.title = $("meta[property='og:title']").attr("content") || "None";
        this.image = $("meta[property='og:image']").attr("content") || "None";
        this.duration = $("meta[property='og:video:duration']").attr("content") || "0";
        this.views = $("span.video_view_count").text() || "None";
        this.rating = $("div.rating_percent.js_rating_percent").attr("data-percent") + "%" || "None";
        this.publish = $("span.video-infobox-date-added").text().replace("Published on ", "") || "None";
        this.upVote = this.rating;
        this.downVote = null;
        this.video = $("meta[name='twitter:player']").attr("content") || "None";
        this.tags = $("a.item.video_carousel_item.video_carousel_category, a.item.video_carousel_item.video_carousel_tag")
          .map((i, el) => {
            return $(el).text();
          }).get();
        this.tags = this.tags.map((el) => lust.removeHtmlTagWithoutSpace(el));
        this.models = $("div.pornstar-name.pornstarPopupWrapper")
          .find("a")
          .map((i, el) => {
            return $(el).text();
          }
          ).get();
        this.models = this.models.map((el) => lust.removeHtmlTagWithoutSpace(el));
        this.models = this.models.filter((el) => !el.includes("Subscribe") && !el.includes("Rank"))
          .filter((el, i, arr) => arr.indexOf(el) === i);
      }
    }
    
    const red = new RedTube();
    const data: IVideoData = {
      success: true,
      data: {
        title: lust.removeHtmlTagWithoutSpace(red.title),
        id: red.id,
        image: red.image,
        duration: lust.secondToMinute(Number(red.duration)),
        views: red.views,
        rating: red.rating,
        uploaded: red.publish,
        upvoted: red.upVote,
        downvoted: red.downVote,
        models: red.models,
        tags: red.tags
      },
      source: red.link,
      assets: [red.video, red.image]
    };
    return data;
  } catch (err) {
    const e = err as Error;
    throw Error(e.message);
  }
}