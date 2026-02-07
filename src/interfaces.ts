export interface IVideoData {
    success: boolean;
    data: {
        title: string;
        id: string;
        image?: string;
        duration: string;
        views: string;
        rating?: string;
        uploaded: string;
        upvoted: string | null;
        downvoted: string | null;
        channel?: string;
        models: string[];
        tags: string[];
    };
    source: string; 
    assets: string[];
}

export interface ISearchVideoData {
    success: boolean;
    data: string[];
    source: string;
}

export interface ISearchItem {
  link: string;
  id: string;
  title?: string;
  image?: string;
  duration?: string;
  rating?: string;
  views?: string;
  uploader?: string;
  video?: string;
}


export interface MaybeError {
    message: string;
}

