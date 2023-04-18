import { load } from "cheerio";
import LustPress from "../../LustPress";
import { IVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  try {
    const resolve = await lust.fetchBody(url);
    const $ = load(resolve);

    class Xhamster { 
      link: string;
      id: string;
      title: string;
      image: string;
      duration: any;
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
        this.id = this.link.split("/")[3] + "/" + this.link.split("/")[4] || "None";
        this.title = $("meta[property='og:title']").attr("content") || "None";
        this.image = $("meta[property='og:image']").attr("content") || "None"; 
        this.duration = $("script#initials-script").html() || "None";
        //remove window.initials={ and };
        this.duration = this.duration.replace("window.initials=", "");
        this.duration = this.duration.replace(/;/g, "");
        this.duration = JSON.parse(this.duration);
        this.duration = this.duration.videoModel.duration || "None";
        this.views = $("div.header-icons").find("span").first().text() || "None";
        this.rating = $("div.header-icons").find("span").eq(1).text() || "None";
        this.publish = $("div.entity-info-container__date").attr("data-tooltip") || "None";
        this.upVote = $("div.rb-new__info").text().split("/")[0].trim() || "None";
        this.downVote = $("div.rb-new__info").text().split("/")[1].trim() || "None";
        this.video = "https://xheve2.com/embed/" + this.link.split("-").pop() || "None";
        this.tags = $("a.video-tag")
          .map((i, el) => {
            return $(el).text();
          }).get();
        this.tags = this.tags.map((el) => lust.removeHtmlTagWithoutSpace(el));
        this.models = $("a.video-tag")
          .map((i, el) => {
            return $(el).attr("href");
          }
          ).get();
        this.models = this.models.filter((el) => el.startsWith("https://xheve2.com/pornstars/"));
        this.models = this.models.map((el) => el.replace("https://xheve2.com/pornstars/", ""));
      }
    }
    
    const xh = new Xhamster();
    const data: IVideoData = {
      success: true,
      data: {
        title: lust.removeHtmlTagWithoutSpace(xh.title),
        id: xh.id,
        image: xh.image,
        duration: lust.secondToMinute(Number(xh.duration)),
        views: xh.views,
        rating: xh.rating,
        uploaded: xh.publish,
        upvoted: xh.upVote,
        downvoted: xh.downVote,
        models: xh.models,
        tags: xh.tags
      },
      source: xh.link,
      assets: [xh.video, xh.image]
    };
    return data;
    
  } catch (err) {
    const e = err as Error;
    throw Error(e.message);
  }
}