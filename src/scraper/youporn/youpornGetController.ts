import { load } from "cheerio";
import LustPress from "../../LustPress";
import { IVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  try {
    const resolve = await lust.fetchBody(url);
    const $ = load(resolve);

    class YouPorn { 
      link: string;
      id: string;
      title: string;
      image: string;
      duration: string;
      views: string;
      rating: string;
      publish: string;
      upVote: string;
      downVote: string;
      video: string;
      tags: string[];
      models: string[];
      constructor() {
        this.link = $("link[rel='canonical']").attr("href") || "None";
        this.id = this.link.replace("https://www.youporn.com/watch/", "") || "None";
        this.title = $("meta[property='og:title']").attr("content") || "None";
        this.image = $("meta[property='og:image']").attr("content") || "None";
        this.duration = $("meta[property='video:duration']").attr("content") || "0";
        this.views = $("div.feature.infoValueBlock").find("div[data-value]").attr("data-value") || "0";
        this.rating = $("div.feature").find("span").text().replace(/[^0-9.,%]/g, "") || "0";
        this.publish = $("div.video-uploaded").find("span").text() || "None";
        this.upVote = this.views;
        this.downVote = "None";
        this.video = `https://www.youporn.com/embed/${this.id}`;
        this.tags = $("a[data-espnode='category_tag'], a[data-espnode='porntag_tag']")
          .map((i, el) => {
            return $(el).text();
          }).get();
        this.models = $("a[data-espnode='pornstar_tag']")
          .map((i, el) => {
            return $(el).text();
          }).get();       
      }
    }
    
    const yp = new YouPorn();
    const data: IVideoData = {
      success: true,
      data: {
        title: lust.removeHtmlTagWithoutSpace(yp.title),
        id: yp.id,
        image: yp.image,
        duration: lust.secondToMinute(Number(yp.duration)),
        views: yp.views,
        rating: yp.rating,
        uploaded: yp.publish,
        upvoted: yp.upVote,
        downvoted: yp.downVote,
        models: yp.models,
        tags: yp.tags
      },
      source: yp.link,
      assets: [yp.video, yp.image]
    };
    return data;
  } catch (err) {
    const e = err as Error;
    throw Error(e.message);
  }
}