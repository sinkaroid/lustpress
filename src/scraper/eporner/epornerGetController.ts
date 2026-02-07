import { load } from "cheerio";
import LustPress from "../../LustPress";
import { IVideoData } from "../../interfaces";

const lust = new LustPress();

function calculateRatingFromStrings(
  upVote: string,
  downVote: string
): number {
  const up = parseInt(upVote.replace(/,/g, ""), 10) || 0;
  const down = parseInt(downVote.replace(/,/g, ""), 10) || 0;

  const total = up + down;
  if (total === 0) return 0;

  return up / total;
}

export async function scrapeContent(url: string) {
  try {
    const resolve = await lust.fetchBody(url);
    const $ = load(resolve);

    class EPorner { 
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
        // https://www.eporner.com/video-ibvqvezXzcs/ivy-seduces-her-dad-s-friend/

        this.link = $("link[rel='canonical']").attr("href") || "None";
        this.id = this.link.split("/")[3] + "/" + this.link.split("/")[4] || "None";

        this.title = $("meta[property='og:title']").attr("content") || "None";
        this.image = $("meta[property='og:image']").attr("content") || "None";
        this.duration = $("meta[property='og:duration']").attr("content") || "0";
        this.views = $("#cinemaviews1").text().trim() || "0";
        this.upVote = $(".likeup i").first().text().trim() || "0";
        this.downVote = $(".likedown i").first().text().trim() || "0";
        
        const jsonLdText = $('script[type="application/ld+json"]').first().html() || "{}";
        const jsonLd = JSON.parse(jsonLdText);
        const isoDuration = jsonLd.duration; // "PT00H8M12S"
        this.publish = jsonLd.uploadDate || "None";

        const ratingValue = calculateRatingFromStrings(this.upVote, this.downVote);
        this.rating = (ratingValue * 100).toFixed(2);

        const videoPart = this.link.split("/")[3] || "None";
        const code = videoPart.replace("video-", "");
        this.video = `https://www.eporner.com/embed/${code}/`;

        this.tags = $("li.vit-category a")
        .map((_, el) => $(el).text().trim()).get();

        this.models = $("li.vit-pornstar a")
        .map((_, el) => $(el).text().trim()).get();
      }
    }
    
    const ep = new EPorner();
    const data: IVideoData = {
      success: true,
      data: {
        title: lust.removeHtmlTagWithoutSpace(ep.title),
        id: ep.id,
        image: ep.image,
        duration: lust.secondToMinute(Number(ep.duration)),
        views: ep.views,
        rating: ep.rating,
        uploaded: ep.publish,
        upvoted: ep.upVote,
        downvoted: ep.downVote,
        models: ep.models,
        tags: ep.tags
      },
      source: ep.link,
      assets: [ep.video, ep.image]
    };
    return data;
  } catch (err) {
    const e = err as Error;
    throw Error(e.message);
  }
}