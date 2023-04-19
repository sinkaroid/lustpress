import { load } from "cheerio";
import LustPress from "../../LustPress";
import { IVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  try {
    const resolve = await lust.fetchBody(url);
    const $ = load(resolve);

    class Xvideos { 
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
      thumbnail: string;
      bigimg: string;
      embed: string;
      constructor() {
        this.link = $("meta[property='og:url']").attr("content") || "None";
        this.id = this.link.split("/")[3] + "/" + this.link.split("/")[4] || "None";
        this.title = $("meta[property='og:title']").attr("content") || "None";
        this.image = $("meta[property='og:image']").attr("content") || "None";
        this.duration = $("meta[property='og:duration']").attr("content") || "0";
        this.views = $("div#v-views").find("strong.mobile-hide").text() || "None";
        this.rating = $("span.rating-total-txt").text() || "None";
        this.publish = $("script[type='application/ld+json']").text() || "None";
        this.publish = this.publish
          .split("uploadDate")[1]
          .split("}")[0]
          .split(":")[1]
          .replace(/"/g, "")
          .replace(/,/g, "") || "None";
        this.upVote = $("span.rating-good-nbr").text() || "None";
        this.downVote = $("span.rating-bad-nbr").text() || "None";
        const thumb = $("script")
          .map((i, el) => {
            return $(el).text();
          }).get()
          .filter((el) => el.includes("html5player.setThumbSlideBig"))[0] || "None";
        this.thumbnail = thumb.match(/html5player.setThumbSlideBig\((.*?)\)/)?.[1] || "None";
        this.bigimg = thumb.match(/html5player.setThumbUrl169\((.*?)\)/)?.[1] || "None";
        this.video = thumb.match(/html5player.setVideoUrlHigh\((.*?)\)/)?.[1] || "None";
        this.tags = $("a.is-keyword.btn.btn-default")
          .map((i, el) => {
            return $(el).text();
          }).get();
        this.models = $("li.model")
          .map((i, el) => {
            return $(el).find("a").attr("href") || "None";
          }
          ).get();
        this.models = this.models.map((el) => el.split("/")[2]);
        this.embed = $("input#copy-video-embed").attr("value") || "None";
        this.embed = this.embed.split("iframe")[1].split(" ")[1].replace(/src=/g, "").replace(/"/g, "") || "None";
      }
    }
    
    const xv = new Xvideos();
    const data: IVideoData = {
      success: true,
      data: {
        title: lust.removeHtmlTagWithoutSpace(xv.title),
        id: xv.id,
        image: xv.image,
        duration: lust.secondToMinute(Number(xv.duration)),
        views: lust.removeHtmlTag(xv.views),
        rating: xv.rating,
        uploaded: xv.publish,
        upvoted: xv.upVote,
        downvoted: xv.downVote,
        models: xv.models,
        tags: xv.tags,
      },
      source: xv.link,
      assets: lust.removeAllSingleQuoteOnArray([xv.embed, xv.thumbnail, xv.bigimg, xv.video])
    };
    return data;
    
  } catch (err) {
    const e = err as Error;
    throw Error(e.message);
  }
}