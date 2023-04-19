import { load } from "cheerio";
import LustPress from "../../LustPress";
import { IVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  try {
    const resolve = await lust.fetchBody(url);
    const $ = load(resolve);

    class Xnxx { 
      link: string;
      id: string;
      title: string;
      image: string;
      duration: string;
      views: string;
      uploaded: string;
      action: string[];
      upVote: string;
      downVote: string;
      favVote: string;
      tags: string[];
      models: string[];
      thumbnail: string;
      bigimg: string;
      video: string;
      embed: string;
      constructor() {
        const thumb = $("script")
          .map((i, el) => {
            return $(el).text();
          }).get()
          .filter((el) => el.includes("html5player.setThumbSlideBig"))[0] || "None";

        this.thumbnail = thumb.match(/html5player.setThumbSlideBig\((.*?)\)/)?.[1] || "None";
        this.bigimg = thumb.match(/html5player.setThumbUrl169\((.*?)\)/)?.[1] || "None";
        this.video = thumb.match(/html5player.setVideoUrlHigh\((.*?)\)/)?.[1] || "None";
        this.link = $("meta[property='og:url']").attr("content") || "None";
        this.id = this.link.split(".com/")[1] || "None";
        this.title = $("meta[property='og:title']").attr("content") || "None";
        this.image = $("meta[property='og:image']").attr("content") || "None";
        this.duration = $("meta[property='og:duration']").attr("content") || "None";
        this.views = $("span.metadata").text() || "None";
        this.views = this.views.split("-")[2] || "None";
        this.uploaded = $("script[type='application/ld+json']").text() || "None";
        this.uploaded = this.uploaded
          .split("uploadDate")[1]
          .split("}")[0]
          .split(":")[1]
          .replace(/"/g, "")
          .replace(/,/g, "") || "None";
        this.action = $("span.vote-actions")
          .find("span.value")
          .map((i, el) => {
            return $(el).text();
          }).get();

        this.upVote = this.action[0] || "None";
        this.downVote = this.action[1] || "None";
        this.favVote = $("span.rating-box.value").text() || "None";
        this.models = $("a.is-pornstar")
          .map((i, el) => {
            return $(el).text();
          }).get();
        this.tags = $("div.metadata-row.video-tags")
          .find("a")
          .map((i, el) => {
            return $(el).text();
          }).get();
        this.embed = $("input#copy-video-embed").attr("value") || "None";
        this.embed = this.embed.split("iframe")[1].split(" ")[1].replace(/src=/g, "").replace(/"/g, "") || "None";

      }
    }
    
    const x = new Xnxx();

    const data: IVideoData = {
      success: true,
      data: {
        title: lust.removeHtmlTagWithoutSpace(x.title),
        id: x.id,
        image: x.image,
        duration: lust.secondToMinute(Number(x.duration)),
        views: lust.removeHtmlTag(x.views),
        rating: x.favVote,
        uploaded: x.uploaded.trim(),
        upvoted: x.upVote,
        downvoted: x.downVote,
        models: x.models,
        tags: x.tags.filter((el) => el !== "Edit tags and models")
      },
      source: x.link,
      assets: lust.removeAllSingleQuoteOnArray([x.embed, x.thumbnail, x.bigimg, x.video])
    };
    return data;
    
  } catch (err) {
    const e = err as Error;
    throw Error(e.message);
  }
}